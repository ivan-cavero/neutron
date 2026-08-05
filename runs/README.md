# runs/ — Historial de runs

Cada run de trabajo queda registrado como `runs/run-NNN.md`. Es la memoria de qué se ha hecho y qué falta: el prompt de generación (OPERATIONS.md §6) lee esta carpeta + STATE.md para decidir el siguiente run.

## Formato de cada run

```markdown
# RUN NNN — <nombre corto>

## Objetivo
<una frase: qué debe ser verdad al terminar>

## Bar
- <referencia real e innegociable: checksum, benchmark, server real, test>

## Tareas
### T1 — <título>
- Qué: ...
- AC: ...
- Evidencia: ...
- DoD: ...
### T2 — ...

## Presupuesto
<orientativo: tokens / tiempo>

## Estado
- [ ] pendiente / [ ] en curso / [x] terminado — <fecha>
- Resultado: <evidencia, números, enlaces>
```

## Reglas

- Numeración correlativa: run-000, run-001, ...
- Nunca editar un run pasado salvo para actualizar su **Estado**.
- Un run solo se marca "terminado" cuando el critic dio PASS o el fallo quedó documentado con evidencia.