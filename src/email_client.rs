use crate::domain::subscriber_email::SubscriberEmail;
use reqwest::{Client, Url};
use secrecy::{ExposeSecret, SecretBox};
use serde::Serialize;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    server_token: SecretBox<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    text_body: &'a str,
    html_body: &'a str,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        server_token: SecretBox<String>,
        timeout_milliseconds: u64,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_milliseconds))
            .build()
            .unwrap();
        Self {
            http_client,
            base_url: base_url,
            sender: sender,
            server_token: server_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        raw_content: &str,
        http_content: &str,
    ) -> Result<(), reqwest::Error> {
        let base_url = Url::parse(self.base_url.as_str()).unwrap();
        let url = base_url.join("email").unwrap();

        let request = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject: subject,
            text_body: raw_content,
            html_body: http_content,
        };

        let builder = self
            .http_client
            .post(url)
            .header("X-Server-Token", self.server_token.expose_secret())
            .json(&request);
        let response = builder.send().await?;
        response.error_for_status()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use claim::assert_err;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use wiremock::matchers::{any, header, header_exists, method, path};
    use wiremock::{Match, Mock, MockServer, ResponseTemplate};

    struct SendEmailRequestMatch;
    impl Match for SendEmailRequestMatch {
        fn matches(&self, request: &wiremock::Request) -> bool {
            let result: Result<serde_json::Value, _> =
                serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("TextBody").is_some()
                    && body.get("HtmlBody").is_some()
            } else {
                false
            }
        }
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::try_from(SafeEmail().fake::<String>()).unwrap()
    }

    fn subject() -> String {
        Sentence(4..8).fake()
    }

    fn content() -> String {
        Paragraph(1..3).fake()
    }

    fn email_client(uri: String) -> EmailClient {
        EmailClient::new(uri, email(), SecretBox::new(Faker.fake()), 10_000)
    }

    #[tokio::test]
    async fn test_send_valid_email_request() {
        let mock_server = MockServer::start().await;

        let email_client = email_client(mock_server.uri());

        Mock::given(header_exists("X-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(method("POST"))
            .and(SendEmailRequestMatch)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;
    }

    #[tokio::test]
    async fn test_failed_if_return_500_response() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let response = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(response);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(180)),
            )
            .mount(&mock_server)
            .await;
        let response = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(response);
    }
}
