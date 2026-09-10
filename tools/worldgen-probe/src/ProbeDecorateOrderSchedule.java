import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IntSummaryStatistics;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.function.IntConsumer;
import java.util.function.IntSupplier;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ChunkLevel;
import net.minecraft.server.level.ChunkTaskPriorityQueue;
import net.minecraft.server.level.ChunkTaskDispatcher;
import net.minecraft.server.level.ChunkTracker;
import net.minecraft.server.level.FullChunkStatus;
import net.minecraft.server.level.Ticket;
import net.minecraft.server.level.TicketType;
import net.minecraft.util.thread.TaskScheduler;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.status.ChunkDependencies;
import net.minecraft.world.level.chunk.status.ChunkPyramid;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.chunk.status.ChunkStep;

/**
 * PROBE DecorateOriginOrder — predicts the order in which overworld chunk ORIGINS
 * execute the biome-decoration pass (applyBiomeDecoration, i.e. FEATURES step)
 * under the canonical reference procedure:
 *
 *   headless server, view-distance=10, no players online (headless => no PLAYER_* tickets)
 *   t=0        : RCON forceload add -128 -128 127 127  -> chunks cx,cz in [-8..7]^2
 *                (ForceLoadCommand.changeForceLoad iterates x OUTER asc, z INNER asc,
 *                 ForceLoadCommand.java:151-181)
 *   t=3000     : four side strips in listed order (west, east, north, south), covering
 *                chunks [-12..11]^2
 *   (150 s / 240 s walls are >> completion time; modeled as two command ticks.)
 *
 * Everything structural uses the REAL 26.2 classes on the extracted classpath:
 *   - ChunkPyramid.GENERATION_PYRAMID / ChunkStep / ChunkDependencies : dependency radii
 *   - ChunkLevel : levels, MAX_LEVEL, generationStatus mapping
 *   - Ticket / TicketType.FORCED : ticket model (FORCED_TICKET_LEVEL is replicated below
 *     because ChunkMap.FORCED_TICKET_LEVEL is public but pulls nothing heavy; we cite it)
 *   - ChunkTaskPriorityQueue + ChunkTaskDispatcher (+ PriorityConsecutiveExecutor /
 *     StrictQueue.FixedPriorityQueue used inside) : THE queue whose pop order we study
 *   - ChunkTracker (real DynamicGraphMinFixedPoint propagation algorithm) subclassed
 *     exactly along LoadingChunkTracker.java:5-49 seams
 * Simulated-only seams (all single-threaded-main-thread actions in vanilla, cited):
 *   - TicketStorage.addTicket listener glue            TicketStorage.java:154-192
 *   - ServerLevel.setChunkForced -> getChunk(FULL)     ServerLevel.java:1506-1512
 *   - getChunkFutureMainThread UNKNOWN@33 add+task     ServerChunkCache.java:230-258
 *     (UNKNOWN@33 never beats FORCED@31 here; modeled implicitly by unconditional
 *      task creation, deviation noted in output header)
 *   - DistanceManager.runAllUpdates order              DistanceManager.java:65-108
 *   - ChunkMap.onLevelChange wiring                    ChunkMap.java:400-403
 *   - ChunkMap.scheduleGenerationTask / runGenerationTasks FIFO pending list
 *                                                      ChunkMap.java:132,659-679
 *   - ChunkGenerationTask layer walk (one LAYER per dispatcher poll; see MODEL notes)
 *                                                      ChunkGenerationTask.java:43-57,59-72,115-129
 *   - worker pool replaced by a single FIFO ManualExecutor drained explicitly per tick:
 *     sequential-fair idealization of Util.backgroundExecutor() with
 *     maxAllowedExecutorThreads() = clamp(nproc-1,1,255) workers (Util.java:229-249).
 *     One dispatcher-poll == one ChunkGenerationTask.runUntilWait() segment advancing
 *     exactly one status LAYER of its center chunk, then re-submitting to the tail of
 *     its queue level bucket — the backlog behaviour of the real server where every
 *     remaining layer is gated by an async future (noise/light) completed elsewhere.
 */
public class ProbeDecorateOrderSchedule {
    // ---- constants mirrored from decompile ----------------------------------
    // ChunkMap.java:128  FORCED_TICKET_LEVEL = ChunkLevel.byStatus(ENTITY_TICKING) = 31
    static int FORCED_TICKET_LEVEL;
    // LoadingChunkTracker.java:6  MAX_LEVEL = ChunkLevel.MAX_LEVEL + 1
    static int TRACKER_MAX_LEVEL;

    static final int TICK_CORE = 0;
    static final int TICK_STRIPS = 150 * 20; // canonical 150 s pause

    // ---- simulation state ----------------------------------------------------
    static final Map<Long, Holder> holders = new HashMap<>();
    static final Set<Long> changedHolders = new HashSet<>();          // DistanceManager.chunksToUpdateFutures analogue
    static final List<Runnable> pendingGenerationTasks = new ArrayList<>(); // ChunkMap.java:132
    static final List<Event> events = new ArrayList<>();
    static final Set<Long> decoratedSet = new HashSet<>();
    static int decoratedSeq = 0;

    // ---- worldgen dispatcher (REAL ChunkTaskDispatcher over ManualExecutor) --
    static ManualExecutor exec;
    static WorldgenSimDispatcher worldgenDispatcher;
    // Fake old-level supplier/setter reaching our holder map, i.e. ChunkHolder::getQueueLevel/setQueueLevel (ChunkHolder.java:246-252)

    static final class Holder {
        final long packed;
        int ticketLevel;
        int queueLevel;
        int completed = -1; // highest finished ChunkStatus index (futures array analogue)
        Holder(long p, int lvl) { this.packed = p; this.ticketLevel = lvl; this.queueLevel = lvl; }
    }

    static final class Event {
        final int tick; final int seq; final String kind; final int cx; final int cz;
        Event(int t, int s, String k, int cx, int cz) { this.tick = t; this.seq = s; this.kind = k; this.cx = cx; this.cz = cz; }
        public String toString() { return String.format(Locale.ROOT, "tick=%5d seq=%6d %-14s (%3d,%3d)", tick, seq, kind, cx, cz); }
    }

    /** Single FIFO executor: drains ONLY when asked, so each simulated tick has phases. */
    static final class ManualExecutor implements TaskScheduler<Runnable>, java.util.concurrent.Executor {
        final String name;
        final List<Runnable> q = new ArrayList<>();
        ManualExecutor(String name) { this.name = name; }
        @Override public String name() { return name; }
        @Override public void schedule(Runnable r) { q.add(r); }
        @Override public void execute(Runnable r) { q.add(r); }
        @Override public Runnable wrapRunnable(Runnable r) { return r; }
        void drain() {
            int guard = 0;
            while (!q.isEmpty()) {
                Runnable r = q.remove(0);
                r.run();
                if (++guard > 20_000_000) throw new IllegalStateException("executor livelock");
            }
        }
    }

    /**
     * ChunkTaskDispatcher subclass adding instrumentation ONLY.
     * (dispatcher mechanics, queue, sleeping/poll cycles are untouched real code)
     */
    static final class WorldgenSimDispatcher extends ChunkTaskDispatcher {
        WorldgenSimDispatcher(TaskScheduler<Runnable> executor, java.util.concurrent.Executor dispatcherExecutor) {
            super(executor, dispatcherExecutor);
        }
        @Override protected ChunkTaskPriorityQueue.TasksForChunk popTasks() {
            ChunkTaskPriorityQueue.TasksForChunk t = super.popTasks();
            return t;
        }
    }

    /**
     * LoadingChunkTracker seam replica (LoadingChunkTracker.java:17-44) over plain maps,
     * running the REAL DynamicGraphMinFixedPoint propagation inherited from ChunkTracker.
     */
    static final class SimLoadingTracker extends ChunkTracker {
        SimLoadingTracker() { super(TRACKER_MAX_LEVEL + 1, 16, 256); } // :11 super(MAX_LEVEL + 1, 16, 256)

        /** LoadingChunkTracker.runDistanceUpdates (:46-48). */
        int runDistanceUpdates(int count) { return runUpdates(count); }

        @Override protected int getLevelFromSource(long to) { return loadTicketLevelAt(to); } // :18-20

        @Override protected int getLevel(long node) {
            Holder h = holders.get(node);
            if (h != null) return h.ticketLevel; // "not to be removed" always true here
            return TRACKER_MAX_LEVEL;             // :31 MAX_LEVEL default
        }

        @Override protected void setLevel(long node, int level) { // :35-44
            Holder h = holders.get(node);
            int oldLevel = h == null ? TRACKER_MAX_LEVEL : h.ticketLevel;
            if (oldLevel != level) {
                if (h == null) { h = new Holder(node, level); holders.put(node, h); }
                else h.ticketLevel = level;
                changedHolders.add(node);
            }
        }
    }

    static SimLoadingTracker tracker;

    /** TicketStorage.getTicketLevelAt(tickets,false)+getLowestTicket replica (:196-215): min level over doesLoad() tickets. */
    static final Map<Long, List<Ticket>> ticketStore = new HashMap<>();
    static int loadTicketLevelAt(long key) {
        List<Ticket> ts = ticketStore.get(key);
        if (ts == null) return ChunkLevel.MAX_LEVEL + 1;
        int min = ChunkLevel.MAX_LEVEL + 1;
        for (Ticket t : ts) if (t.getType().doesLoad() && t.getTicketLevel() < min) min = t.getTicketLevel();
        return min;
    }
    static void updateChunkForced(int cx, int cz, boolean forced) { // TicketStorage.java:397-400
        long key = ChunkPos.pack(cx, cz);
        Ticket t = new Ticket(TicketType.FORCED, FORCED_TICKET_LEVEL);
        List<Ticket> ts = ticketStore.computeIfAbsent(key, k -> new ArrayList<>());
        boolean dup = false;
        for (Ticket o : ts) dup |= o.getType() == t.getType() && o.getTicketLevel() == t.getTicketLevel();
        if (!dup && forced) {
            ts.add(t);
            // addTicket listener glue (:171-176): immediately push decrease into the tracker
            tracker.update(key, t.getTicketLevel(), true);
        }
    }

    // ---- ChunkGenerationTask replica (layered) -------------------------------
    static ChunkPyramid PYRAMID;
    static ChunkStep FULL_STEP;

    static void initMcStatics() {
        FORCED_TICKET_LEVEL = ChunkLevel.byStatus(FullChunkStatus.ENTITY_TICKING);
        TRACKER_MAX_LEVEL = ChunkLevel.MAX_LEVEL + 1;
        tracker = new SimLoadingTracker();
        PYRAMID = ChunkPyramid.GENERATION_PYRAMID;
        FULL_STEP = PYRAMID.getStepTo(ChunkStatus.FULL);
    }

    static final class GenerationTaskSim implements Runnable {
        final int cx, cz;
        ChunkStatus scheduledStatus = null;
        GenerationTaskSim(int cx, int cz) { this.cx = cx; this.cz = cz; }

        boolean cancelled() { return false; }

        /** @return false if a dependency cell cannot advance right now (task parks until its prerequisite completes elsewhere). */
        boolean tryAdvanceOneLayer() {
            if (cancelled() || reachedTarget()) return true;
            ChunkStatus next = scheduledStatus == null
                ? ChunkStatus.EMPTY
                : ChunkStatus.getStatusList().get(scheduledStatus.getIndex() + 1);
            int radius = FULL_STEP.getAccumulatedRadiusOf(next); // getRadiusForLayer, ChunkGenerationTask.java:131-134
            for (int dx = -radius; dx <= radius; dx++) {          // :120 x outer ASC
                for (int dz = -radius; dz <= radius; dz++) {      // :121 z inner ASC
                    if (!apply(next, cx + dx, cz + dz)) return false;
                }
            }
            scheduledStatus = next;
            return true;
        }

        /**
         * One apply == inline ChunkMap.applyStep (ChunkMap.java:630-655):
         * target cell must already hold the PARENT status ("Parent chunk missing" check),
         * and the cell's ticket level must permit this status at all
         * (GenerationChunkHolder.highestAllowedStatus / isStatusDisallowed :229-232,
         *  status source ChunkLevel.generationStatus :17-19).
         */
        boolean apply(ChunkStatus status, int ax, int az) {
            long key = ChunkPos.pack(ax, az);
            // StaticCache2D/acquireGeneration upfront working set (ChunkGenerationTask.java:35-41):
            // the task materializes holders ±RADIUS_AROUND_FULL_CHUNK around ITS center even
            // where no ticket ever reaches; their LEVEL still gates how far statuses may go.
            Holder h = holders.computeIfAbsent(key, k -> new Holder(k, steadyStateLoadLevel(k)));
            ChunkStatus reachable = ChunkLevel.generationStatus(h.ticketLevel);
            if (reachable == null || reachable.isBefore(status)) return false;  // level too shallow: park until tickets grow
            if (status != ChunkStatus.EMPTY && h.completed < status.getParent().getIndex()) return false;
            h.completed = Math.max(h.completed, status.getIndex());             // acquireStatusBump, monotone CAS analogue
            if (status == ChunkStatus.FEATURES) recordDecorate(ax, az);
            return true;
        }

        boolean reachedTarget() { return scheduledStatus == ChunkStatus.FULL; }

        @Override public void run() { // one poll == one layer (MODEL rule; see class doc)
            boolean advanced = tryAdvanceOneLayer();
            if (reachedTarget()) return;
            if (advanced) resubmitSelfTail();
            else parkedTasks.add(this);
        }

        /** ChunkMap.runGenerationTask continuation (ChunkMap.java:667-674). */
        void resubmitSelfTail() {
            Holder h = requireHolder(ChunkPos.pack(cx, cz));
            worldgenDispatcher.submit(this, ChunkPos.pack(cx, cz), qs(h));
        }
    }

    static final List<GenerationTaskSim> parkedTasks = new ArrayList<>();

    static Holder requireHolder(long key) {
        Holder h = holders.get(key);
        if (h == null) throw new IllegalStateException("holder missing " + ChunkPos.unpack(key));
        return h;
    }

    static IntSupplier qs(Holder h) { return () -> h.queueLevel; }

    static void recordDecorate(int cx, int cz) {
        long key = ChunkPos.pack(cx, cz);
        Holder h = holders.get(key);
        if (Boolean.getBoolean("probe.debug")) {
            System.err.printf(Locale.ROOT, "DEBUG claim (%d,%d) ticketLevel=%d completed=%d claimingCenterUnknown%n",
                cx, cz, h == null ? -1 : h.ticketLevel, h == null ? -1 : h.completed);
        }
        if (decoratedSet.add(key)) {
            events.add(new Event(curTick, ++decoratedSeq, "FEATURES", cx, cz));
        } else {
            events.add(new Event(curTick, -1, "FEATURES-dup", cx, cz));
        }
    }

    static int curTick = 0;

    // ---- canonical command ingestion ----------------------------------------
    static boolean ingestCommands(int tick) {
        List<int[]> coords = new ArrayList<>();
        if (tick == TICK_CORE) {
            addRange(coords, -8, 7, -8, 7);
        } else if (tick == TICK_STRIPS) {
            addRange(coords, -12, -11, -12, 11); // west strip  -192..-161 blocks x
            addRange(coords, 10, 11, -12, 11);   // east strip   160..191 blocks x
            addRange(coords, -10, 9, -12, -11);  // north strip -192..-161 blocks z
            addRange(coords, -10, 9, 10, 11);    // south strip  160..191 blocks z
        } else return false;
        for (int[] c : coords) {
            int cx = c[0], cz = c[1];
            updateChunkForced(cx, cz, true);                                  // + immediate tracker.update
            ServerLevel_setChunkForced_sideEffect(cx, cz);                    // getChunk(FULL) sync trigger
            // queue-level resorts happen in the distance-update phase
        }
        return !coords.isEmpty();
    }

    static void ServerLevel_setChunkForced_sideEffect(int cx, int cz) {
        // ServerLevel.java:1506-1512: if forced && updated -> getChunk(x,z) ->
        // getChunkFutureMainThread(FULL): adds UNKNOWN@33 ticket & schedules generation task
        // (ServerChunkCache.java:230-258); inline distance updates (:245-248) folded into
        // our distance-update phase; UNKNOWN@33 omitted (dominated by FORCED@31).
        pendingGenerationTasks.add(new GenerationTaskSim(cx, cz));           // ChunkMap.java:659-662
    }

    static void addRange(List<int[]> out, int cxMin, int cxMax, int czMin, int czMax) {
        for (int x = cxMin; x <= cxMax; x++)       // ForceLoadCommand.java:171 x outer
            for (int z = czMin; z <= czMax; z++)   //                  :172 z inner
                out.add(new int[]{x, z});
    }

    // ---- one simulated server tick (ServerChunkCache.tick shape) -------------
    static void serverTick(int tick) {
        curTick = tick;
        boolean hadCommands = ingestCommands(tick);           // commands executed on main thread

        // runDistanceManagerUpdates (ServerChunkCache.java:281-291):
        // purgeStaleTickets skipped: FORCED has no timeout, no players, docs deviation noted
        changedHolders.clear();
        tracker.runDistanceUpdates(Integer.MAX_VALUE);         // DistanceManager.java:69
        // refresh auto-created working-set holders to the current propagation steady state
        for (Holder h : holders.values()) {
            int tl = steadyStateLoadLevel(h.packed);
            if (tl != h.ticketLevel) { h.ticketLevel = tl; changedHolders.add(h.packed); }
        }
        // DistanceManager.chunksToUpdateFutures loop -> updateHighestAllowedStatus/updateFutures
        // the observable scheduler effect is the queue-level RESORT (ChunkMap.onLevelChange):
        for (long key : changedHolders) {
            Holder h = holders.get(key);
            dispatchResort(h);
        }
        // promoteChunkMap: no-op in sim (fresh chunks, no unload pressure)

        // chunkMap.runGenerationTasks() (:676-679) — FIFO task-creation order
        for (Runnable r : new ArrayList<>(pendingGenerationTasks)) {
            pendingGenerationTasks.remove(r);
            GenerationTaskSim g = (GenerationTaskSim) r;
            Holder h = requireHolder(ChunkPos.pack(g.cx, g.cz));
            worldgenDispatcher.submit(g, ChunkPos.pack(g.cx, g.cz), qs(h)); // ChunkMap.java:667-673
        }
        if (hadCommands && !parkedTasks.isEmpty()) {           // new tickets wake parked waits
            pendingGenerationTasks.addAll(parkedTasks);
            parkedTasks.clear();
        }

        exec.drain();                                          // workers consume until quiet
    }

    static void dispatchResort(Holder h) {
        // ChunkMap.onLevelChange (ChunkMap.java:400-403): worldgen (+light) dispatchers
        worldgenDispatcher.onLevelChange(ChunkPos.unpack(h.packed), () -> h.queueLevel, h.ticketLevel, nl -> h.queueLevel = nl);
    }

    /**
     * Steady-state of the real propagation for point sources min(31 + cheb distance):
     * exact limit of the DynamicGraphMinFixedPoint fixed point for the FORCED-only field;
     * used on-demand so auto-created working-set holders start with a sane level, and
     * re-applied every tick's distance phase (transient lower values are what park tasks).
     */
    static int steadyStateLoadLevel(long key) {
        int min = TRACKER_MAX_LEVEL;
        for (long src : ticketStore.keySet()) {
            if (!ticketStore.get(src).isEmpty()) {
                int lv = FORCED_TICKET_LEVEL + Math.max(Math.abs(ChunkPos.getX(key) - ChunkPos.getX(src)), Math.abs(ChunkPos.getZ(key) - ChunkPos.getZ(src)));
                min = Math.min(min, lv);
            }
        }
        return min;
    }

    // ---- reporting -----------------------------------------------------------
    static void printStaticTables() {
        System.out.println("=== STATIC TABLES (real classes) ===");
        System.out.printf("RADIUS_AROUND_FULL_CHUNK=%d  ChunkLevel.MAX_LEVEL=%d  PRIORITY_LEVEL_COUNT(ChunkTaskPriorityQueue)=%d%n",
            ChunkLevel.RADIUS_AROUND_FULL_CHUNK, ChunkLevel.MAX_LEVEL, ChunkTaskPriorityQueue.PRIORITY_LEVEL_COUNT);
        System.out.printf("FORCED_TICKET_LEVEL=%d (ChunkMap.java:128=byStatus(ENTITY_TICKING)); byStatus(FULL)=%d%n",
            FORCED_TICKET_LEVEL, ChunkLevel.byStatus(ChunkStatus.FULL));
        ChunkDependencies acc = FULL_STEP.accumulatedDependencies();
        StringBuilder sb = new StringBuilder("FULL-step accumulated dependencies (distance->required):\n");
        for (int d = 0; d < acc.size(); d++) sb.append(String.format("  d=%d:%s%n", d, acc.get(d)));
        System.out.println(sb);
        StringBuilder sb2 = new StringBuilder("layer radii within FULL step (getRadiusForLayer basis):\n");
        for (ChunkStatus s : ChunkStatus.getStatusList())
            sb2.append(String.format("  %-18s r=%d%n", s, FULL_STEP.getAccumulatedRadiusOf(s)));
        System.out.println(sb2);
        StringBuilder sb3 = new StringBuilder("reachable generation status per ring level around FORCED@31 region:\n");
        for (int lv = 30; lv <= 30 + Math.min(acc.size(), 14); lv++)
            sb3.append(String.format("  level=%d (distToFull=%d) -> %s%n", lv, lv - 33, ChunkLevel.generationStatus(lv)));
        System.out.println(sb3);
    }

    static int rankOf(int cx, int cz) {
        long key = ChunkPos.pack(cx, cz);
        if (!decoratedSet.contains(key)) return -1;
        return decoratedSeqOf.getOrDefault(key, -1);
    }
    static final Map<Long, Integer> decoratedSeqOf = new HashMap<>();

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        initMcStatics();

        printStaticTables();

        exec = new ManualExecutor("worldgen");
        worldgenDispatcher = new WorldgenSimDispatcher(exec, exec);
        // (26.2 wires TWO dispatchers over the same executor: worldgen + light, ChunkMap.java:192-197;
        //  light tasks don't gate FEATURES, merged away — documented deviation.)

        int endTick = TICK_STRIPS + 60; // strips settle quickly; guard generous
        for (int t = 0; t <= endTick; t++) serverTick(t);

        // fill seq lookup
        for (Event e : events) if (e.seq > 0) decoratedSeqOf.put(ChunkPos.pack(e.cx, e.cz), e.seq);

        // ---- report ----
        System.out.println("\n=== EVENT LOG (FEATURES executions, chronological) ===");
        int shown = 0;
        for (Event e : events) {
            if (e.kind.equals("FEATURES")) {
                System.out.println(e);
                shown++;
            }
        }
        System.err.println("FULL FEATURES COUNT=" + shown);

        long inRegion = events.stream().filter(e -> e.kind.equals("FEATURES"))
            .filter(e -> -12 <= e.cx && e.cx <= 11 && -12 <= e.cz && e.cz <= 11).count();
        Map<Integer, Integer> hist = new HashMap<>();
        for (Event e : events) if (e.kind.equals("FEATURES")) hist.merge(e.tick, 1, Integer::sum);
        System.out.println("\n=== SUMMARY ===");
        System.out.printf("decorated total(incl ticket halo beyond strips)=%d ; within [-12..11]^2=%d/%d%n",
            decoratedSeqOf.size(), inRegion, 24L * 24L);
        IntSummaryStatistics tickStats = events.stream().filter(e -> e.kind.equals("FEATURES"))
            .filter(e -> -12 <= e.cx && e.cx <= 11 && -12 <= e.cz && e.cz <= 11).mapToInt(e -> e.tick).summaryStatistics();
        System.out.printf("in-region decorate ticks: first=%d last=%d distinctTicks=%s%n",
            tickStats.getMin(), tickStats.getMax(), hist.keySet().stream().sorted().toList());

        System.out.println("first 10 decorate order:");
        events.stream().filter(e -> e.kind.equals("FEATURES")).limit(10).forEach(e -> System.out.println("  " + e));

        System.out.println("special ranks (seq=-1 ⇒ not decorated in window):");
        int[][] specials = {{0, 0}, {-1, 1}, {-8, -8}, {7, 7}, {-11, 0}, {10, 0}, {-12, -12}, {11, 11}, {-10, -11}, {9, 10}};
        for (int[] s : specials)
            System.out.printf("  origin (%3d,%3d) seq=%d%n", s[0], s[1], rankOf(s[0], s[1]));

        System.out.println("tick histograms (features per tick):");
        new ArrayList<>(hist.keySet()).stream().sorted().forEach(k -> System.out.printf("  tick %5d : %d%n", k, hist.get(k)));

        // ring transition: bounding box of newly decorated per tick & cumulative frontier
        System.out.println("frontier trace (cumulative bbox of decorated set when it grows):");
        int minX = Integer.MAX_VALUE, maxX = Integer.MIN_VALUE, minZ = Integer.MAX_VALUE, maxZ = Integer.MIN_VALUE;
        List<String> lines = new ArrayList<>();
        for (Event e : events) {
            if (!e.kind.equals("FEATURES")) continue;
            minX = Math.min(minX, e.cx); maxX = Math.max(maxX, e.cx);
            minZ = Math.min(minZ, e.cz); maxZ = Math.max(maxZ, e.cz);
            lines.add(String.format(Locale.ROOT, "seq=%6d (%3d,%3d) cumBBox=[%d..%d]x[%d..%d]", e.seq, e.cx, e.cz, minX, maxX, minZ, maxZ));
        }
        lines.stream().limit(15).forEach(l -> System.out.println("  " + l));
        lines.subList(Math.max(0, lines.size() - 15), lines.size()).forEach(l -> System.out.println("  ..." + l));

        System.out.println("\nMODEL NOTES:");
        System.out.println(" - one dispatcher poll == ONE status layer advanced (backlog idealization);");
        System.out.println(" - worker pool serialized FIFO (utilizes no parallelism) ⇒ THIS OUTPUT IS THE");
        System.out.println("   DETERMINISTIC BOUNDARY; real 26.2 lets completion callbacks race into the");
        System.out.println("   same queues (see desk-dump determinism verdict), so ring-neighbour pairs");
        System.out.println("   adjacent in this order are the ones that can swap under real threading.");
        System.out.println(" - UNKNOWN@33 synchronous-trigger ticket omitted (dominated by FORCED@31);");
        System.out.println(" - light-task second dispatcher merged (doesn't gate FEATURES).");
    }
}
