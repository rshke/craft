use std::fmt;

use serde::Deserialize;

use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_name::SubscriberName;

#[derive(Deserialize, Debug)]
pub struct Subscriber {
    pub name: SubscriberName,
    pub email: SubscriberEmail,
}

pub enum SubscriberStatus {
    Confirmed,
}

impl fmt::Display for SubscriberStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubscriberStatus::Confirmed => write!(f, "Confirmed"),
        }
    }
}
