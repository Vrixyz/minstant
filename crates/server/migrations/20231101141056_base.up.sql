CREATE TABLE IF NOT EXISTS teams
(
    id serial primary key,
    name varchar(32) not null,
    is_playing boolean not null DEFAULT false
);

create table if not exists champions
(
    id serial primary key,
    points int not null CHECK(points >= 0) DEFAULT 0,
    name varchar(32) not null,
    team_id int references teams (id) on delete cascade
);

create table if not exists points_pool
(
    id serial primary key,
    points int not null CHECK(points >= 0) default 0
);

create table if not exists users
(
    id serial primary key,
    name varchar(32) not null,
    
    points int not null CHECK(points >= 0) DEFAULT 0,
    can_get_points_time timestamp not null DEFAULT NOW(),
    pw_hash varchar(255) not null,

    team_id int references teams (id) on delete set null
);

create table if not exists users_points_history
(
    id serial primary key,
    time timestamp not null DEFAULT now(),

    user_id int references users (id) on delete cascade not null 
);