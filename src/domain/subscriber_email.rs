use std::str::FromStr;
use validator::ValidateEmail;

#[derive(Debug, Clone)]
pub struct SubscriberEmail(String);

impl FromStr for SubscriberEmail {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.validate_email() {
            Ok(Self(s.to_string()))
        } else {
            Err("Invalid email.".to_string())
        }
    }
}

impl TryFrom<String> for SubscriberEmail {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.validate_email() {
            Ok(Self(s.to_string()))
        } else {
            Err("Invalid email.".to_string())
        }
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for SubscriberEmail {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // auto deref
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::SubscriberEmail;
    use claim::assert_err;
    use fake::Fake;

    #[test]
    fn without_at_is_invalid() {
        let invalid_email = "noisy_drop.gmail.com";
        assert_err!(invalid_email.parse::<SubscriberEmail>());
    }

    #[test]
    fn without_subject_is_invalid() {
        let invalid_email = "@gmail.com";
        assert_err!(invalid_email.parse::<SubscriberEmail>());
    }

    #[test]
    fn empty_is_invalid() {
        let invalid_email = "";
        assert_err!(invalid_email.parse::<SubscriberEmail>());
    }

    use fake::faker::internet::en::SafeEmail;
    use rand::{SeedableRng, rngs::StdRng};
    #[derive(Debug, Clone)]
    struct ValidEmailFixture(String);

    impl quickcheck::Arbitrary for ValidEmailFixture {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut rng = StdRng::seed_from_u64(u64::arbitrary(g));
            let email: String = SafeEmail().fake_with_rng(&mut rng);
            Self(email)
        }
    }

    #[quickcheck_macros::quickcheck]
    fn valid_is_valid(valid_email: ValidEmailFixture) -> bool {
        let valid_email = valid_email.0;
        valid_email.parse::<SubscriberEmail>().is_ok()
    }
}
