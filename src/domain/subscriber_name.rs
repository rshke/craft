#[derive(Debug)]
pub struct SubscriberName(String);

impl TryFrom<String> for SubscriberName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let is_empty = value.trim().is_empty();
        let is_too_long = value.len() > 256;
        let forbidden_characters =
            ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_characters =
            value.chars().any(|g| forbidden_characters.contains(&g));

        if is_empty || is_too_long || contains_forbidden_characters {
            return Err("invalid subscriber name".to_string());
        }

        Ok(Self(value))
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for SubscriberName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.try_into().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::SubscriberName;
    use claim::{assert_err, assert_ok};

    #[test]
    fn a_256_grapheme_long_name_is_valid() {
        let name = "a".repeat(256);
        assert_ok!(SubscriberName::try_from(name));
    }

    #[test]
    fn a_name_longer_than_256_graphemes_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::try_from(name));
    }

    #[test]
    fn whitespace_only_names_are_rejected() {
        let name = " ".to_string();
        assert_err!(SubscriberName::try_from(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = "".to_string();
        assert_err!(SubscriberName::try_from(name));
    }

    #[test]
    fn names_containing_forbidden_characters_are_rejected() {
        for name in &['/', '(', ')', '"', '<', '>', '\\', '{', '}'] {
            let name = name.to_string();
            assert_err!(SubscriberName::try_from(name));
        }
    }

    #[test]
    fn a_valid_name_is_parsed_successfully() {
        let name = "Ursula Le Guin".to_string();
        assert_ok!(SubscriberName::try_from(name));
    }
}
