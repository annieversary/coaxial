use std::fmt::{Debug, Display, Write};

use rand::{distributions::Alphanumeric, Rng};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RandomId([u8; 8]);

impl RandomId {
    pub fn from_rng<RNG: Rng>(rng: &mut RNG) -> Self {
        let array = [(); 8].map(|_| rng.sample(Alphanumeric));

        Self(array)
    }

    #[allow(dead_code)]
    pub(crate) fn from_str(string: &str) -> Self {
        let array: [u8; 8] = string.as_bytes()[..8]
            .try_into()
            .expect("provided string was less than 8 characters long");
        Self(array)
    }

    pub fn fmt(&self, output: &mut dyn Write) -> std::fmt::Result {
        for c in self.0 {
            output.write_char(char::from(c))?;
        }

        Ok(())
    }
}

impl Debug for RandomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RandomId(")?;

        Self::fmt(self, f)?;

        f.write_char(')')?;

        Ok(())
    }
}

impl Display for RandomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Self::fmt(self, f)
    }
}
