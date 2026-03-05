# Getting started

`cargo build` to build the solution, if it doesn't work, then let me know asap. I still need to setup CICD

# Style
I hope to put some humanity in the docs, comments, and code here. This project is for fun. So why not smile while doing so?

# Hopes and Dreams
Use as minimal dependencies
Hand crafted, like a delicous beer
AI for the bitch work

# Rant
agentic coding will eventually take over for commercial software development
full stop. it's fast. it can fuck.
I consider myself an artisan, craftsman with code.
Bits and bytes, syntax; to me is like saw dust to a carpenter. We master them.
AI is my powertools.
IDE is my handtools.
Lovingly caring over the feel of every line, this is my refuge from corporate america.
Back in my day... to pass a class, I had to write a doubly linked list, and a stack implementation, in C++, with no GC, no notes, no google, no AI, no docs. Practically hand written from scratch.
I'm trying to keep that vibe alive.

# Architecture
Architecture

 source(s) ──→ ch1 (async_channel) ──→ composer_worker(s) ──→ ch2 (async_channel) ──→ sink_worker(s) ──→ sink
                MPMC, String             std::thread              MPMC, String            tokio::spawn
                bounded(queue_capacity)   blocking_recv/send       bounded(payload_ch_cap)  async recv

Manifolds: Combines a bunch of different things together
Fittings: like an adapter, adapts between different types of pipe types and sizes
