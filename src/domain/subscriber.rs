use serde::Deserialize;

use crate::domain::subscriber_name::SubscriberName;
use crate::domain::subscriber_email::SubscriberEmail;

#[derive(Deserialize, Debug)]
pub struct Subscriber {
    pub name: SubscriberName,
    pub email: SubscriberEmail,
}
