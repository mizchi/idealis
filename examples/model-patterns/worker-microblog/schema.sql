CREATE TABLE follows (reader TEXT NOT NULL, author TEXT NOT NULL, PRIMARY KEY (reader, author));
CREATE TABLE posts (post TEXT PRIMARY KEY, author TEXT NOT NULL);
CREATE TABLE feed (reader TEXT NOT NULL, post TEXT NOT NULL, PRIMARY KEY (reader, post));
CREATE TABLE applied (post TEXT PRIMARY KEY);
