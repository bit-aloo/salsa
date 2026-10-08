# Query operations

The most important basic operations that all queries support are:

* [maybe changed after](./maybe_changed_after.md): Returns true if the value of the query (for the given key) may have changed since the given revision.
* [Fetch](./fetch.md): Returns the up-to-date value for the given K. Panics if the query is part of a cycle that the query has not configured Salsa to recover from (see [cycle handling](../cycles.md)).
