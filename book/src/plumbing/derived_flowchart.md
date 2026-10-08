# Derived queries flowchart

Derived queries are by far the most complex. This flowchart documents the flow of the [maybe changed after] and [fetch] operations. This flowchart can be edited on [draw.io]:

[draw.io]: https://draw.io
[fetch]: ./fetch.md
[maybe changed after]: ./maybe_changed_after.md

**Note:** the cycle handling shown in this flowchart predates fixpoint iteration.
Today, when a query that has configured cycle recovery is re-entered, Salsa returns a provisional initial value instead of unwinding;
see [cycle handling](../cycles.md) for the current behavior.

<!-- The explicit div is there because, otherwise, the flowchart is unreadable when using "dark mode" -->
<div style="background-color:white;">

![Flowchart](../derived-query-read.drawio.svg)

</div>
