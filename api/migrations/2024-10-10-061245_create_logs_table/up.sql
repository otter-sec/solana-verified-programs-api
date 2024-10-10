-- Your SQL goes here

CREATE TABLE build_logs (
    id VARCHAR NOT NULL,
    program_address VARCHAR NOT NULL,
    file_name VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY (program_address)
);

