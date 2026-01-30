-- Add migration script here
ALTER TABLE issue_delivery_queue
ADD COLUMN n_retries SMALLINT NOT NULL DEFAULT 0;

ALTER TABLE issue_delivery_queue
ADD COLUMN execute_after TIMESTAMP
WITH
    TIME zone NOT NULL DEFAULT now ();
