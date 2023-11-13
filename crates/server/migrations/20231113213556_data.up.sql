-- Add up migration script here
insert into teams (name) values('Blue'),('Red'),('Green');
insert into champions (name, team_id)
values
    ('Josh', (select id from teams where name = 'Blue')),
    ('Matthias', (select id from teams where name = 'Red')),
    ('Cecile', (select id from teams where name = 'Green'));

insert into points_pool (points) values(10);