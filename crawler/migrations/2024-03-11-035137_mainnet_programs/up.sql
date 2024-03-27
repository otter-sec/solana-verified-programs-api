-- Your SQL goes here
-- Create a table to store the mainnet programs
CREATE TABLE mainnet_programs (
    id SERIAL PRIMARY KEY,
    project_name VARCHAR,
    program_address VARCHAR UNIQUE NOT NULL,
    buffer_address VARCHAR NOT NULL,
    github_repo VARCHAR,
    has_security_txt BOOLEAN NOT NULL,
    is_closed BOOLEAN DEFAULT false NOT NULL,
    is_success BOOLEAN DEFAULT false NOT NULL,
    is_processed BOOLEAN DEFAULT false NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    last_deployed_slot BIGINT,
    update_authority VARCHAR
);


-- Create a unique index on the program_address column
CREATE UNIQUE INDEX mainnet_programs_program_address_uindex ON mainnet_programs (program_address); 