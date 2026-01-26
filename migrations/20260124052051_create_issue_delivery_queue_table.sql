-- Add migration script here
CREATE TABLE issue_delivery_queue (
    newsletter_issue_id uuid NOT NULL REFERENCES newsletter_issues (newsletter_issue_id),
    subscriber_email text NOT NULL,
    PRIMARY key (newsletter_issue_id, subscriber_email)
)
