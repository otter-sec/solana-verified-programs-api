-- Your SQL goes here

CREATE TABLE build_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    program_address VARCHAR NOT NULL,
    file_name VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now()
);

CREATE INDEX idx_program_address ON build_logs(program_address);
