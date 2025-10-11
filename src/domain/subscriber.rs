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
    PendingConfirmation,
}

impl fmt::Display for SubscriberStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confirmed => write!(f, "confirmed"),
            Self::PendingConfirmation => write!(f, "pending_confirmation"),
        }
    }
}
