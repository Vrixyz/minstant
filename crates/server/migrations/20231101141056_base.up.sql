CREATE TABLE IF NOT EXISTS teams
(
    id serial primary key,
    name varchar(32) not null
);

create table if not exists champions
(
    id serial primary key,
    points int not null CHECK(points >= 0) DEFAULT 0,
    name varchar(32) not null UNIQUE,
    team_id int references teams (id) on delete cascade
);

create table if not exists points_pool
(
    id serial primary key,
    points int not null CHECK(points >= 0) default 1,
    open_at timestamp not null DEFAULT NOW()
);

create table if not exists users
(
    id serial primary key,
    name varchar(32) UNIQUE not null,
    
    points int not null CHECK(points >= 0) DEFAULT 0,
    can_get_points_time timestamp not null DEFAULT NOW(),

    team_id int references teams (id) on delete set null,
    password text NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    session_token BYTEA PRIMARY KEY,
    user_id integer REFERENCES users (id) ON DELETE CASCADE
);