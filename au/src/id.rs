use rand::Rng;
use sqids::Sqids;

pub struct IdGen {
    squids: Sqids,
}

impl Default for IdGen {
    fn default() -> Self {
        IdGen{
            squids: Sqids::builder()
                .min_length(8)
                .alphabet("0123456789ABCDEFGHJKMNPQRSTVWXYZ".chars().collect())
                .build()
                .unwrap()
        }
    }
}

impl IdGen {
    pub fn gen (&self, mut rng: impl Rng) -> String {
        self.squids.encode(&[rng.gen()]).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::id::IdGen;

    #[test]
    fn test_gen() {
        let g = IdGen::default();
        let mtng = rand::thread_rng();
        assert!(g.gen(mtng).len() >= 8);
        assert_ne!(g.gen(mtng), g.gen(mtng));
    }

}
