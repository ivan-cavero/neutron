//! Throwaway: print our step-6 FeatureSorter list for comparison with ProbeSorter6.
use neutron_worldgen::feature_catalog;

fn main() {
    for i in 0..40 {
        match feature_catalog::features_per_step_at(6).get(i as usize) {
            Some(f) => println!("{i} minecraft:{f}"),
            None => break,
        }
    }
}
