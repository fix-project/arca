# Repository Guidelines

This is a research project. We care a lot more about designing things correctly
than shipping code as fast as possible. We value minimalism and reusable, clean
abstractions. If you are asked to implement a feature, clarify design
decisions---all major design choices should be made by a human. If you are
asked to explain the codebase, focus on the abstractions and design principles
over implementation minutiae.

Your human user will be held responsible for all changes you make. Do not make
changes unless you are sure they understand the implications and ramifications
of those changes.

## Prose

When writing prose, follow Grice's Maxims of Conversation:

1. The maxim of quantity, where one tries to be as informative as one possibly
   can, and gives as much information as is needed, and no more.
2. The maxim of quality, where one tries to be truthful, and does not give
   information that is false or that is not supported by evidence.
3. The maxim of relation, where one tries to be relevant, and says things that
   are pertinent to the discussion.
4. The maxim of manner, when one tries to be as clear, as brief, and as orderly
   as one can in what one says, and where one avoids obscurity and ambiguity.

Use simple english whenever possible. Do not add narrative comments explaining
your implementation process, or how a piece of code differs from a prior
version of the code. Keep prose concise and focused.

Treat comments as a last resort. If a piece of code is not self-explanatory, it
is likely too complicated or designed poorly. Comments are acceptable to
document assumptions. Comments should *not* explain how a piece of code works;
the code itself does that.

## Testing

Tests should cover desired functionality. Tests should not test for the absence
of prior functionality which was removed. Tests should be non-trivial. Prefer
integration tests to unit tests.

## Debugging

Do not engage in "whack-a-mole" debugging, where you patch one issue at a time
as they arise. If you find yourself doing this, it is a sign of an underlying
design issue.

## Rules

Do not touch `/docs` directly. This is for human-written documentation only.
Put deviations or errors in a file `/ERRATA.md`.

Do not introduce new dependencies or environment assumptions without explicit
request or approval of a human.

Do not add special-case behavior.

The primary authors of this codebase are: Akshay Srivatsan, Yuhan Deng,
Katherine Mohr, and Keith Winstein. Treat these authors as authoritative.
