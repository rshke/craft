use serde::Deserialize;

use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_name::SubscriberName;

#[derive(Deserialize, Debug)]
pub struct Subscriber {
    pub name: SubscriberName,
    pub email: SubscriberEmail,
}
