---
title: Our system doesn't need a model to do the retrieval
date: 2026-09-03
lang: en
description: The first video on the channel: why I decided to build Ulpia, why AI memory should be local, and why retrieval here does not go through a model.
source: https://youtu.be/ADNbzwcNHHY
---

What's up everyone. I need to take this moment to talk about the project I'm building,
Ulpia. And this is also going to be the first video on the channel. This channel is going to
be focused on talking about technology and other things I find interesting, but mostly we're
going to talk about software development and technology.

This one is going to be quick, but I hope it's informative about Ulpia, and also about the
reason why I decided to build it. So let's go.

I've noticed that artificial intelligence really is part of our lives now, especially for
anyone who works in software development. It's something we can't escape anymore, especially
anyone already working at a production level, anyone already shipping product to the market
and so on. These days there's no way for us to do things purely by hand, coding everything by
hand. These days you need the efficiency that AI helps you get. And of course, following
software engineering fundamentals so you don't ship anything without quality.

## Documentation is one of the main pillars

And one of the things that is obvious, that improves the output of your AI agents a lot, of
your work with artificial intelligence, is documentation. The better documented your system
is, the better any maintenance you do there is going to be, or any implementation of new
features. And that's why I think documentation is one of the main pillars today for anyone
who develops software.

And thinking about that, we get to another point, a point that is correlated to it, that
doesn't necessarily only work for people who develop software. It solves problems for other
people, in other areas, but in particular it solves it for anyone who needs an agent to have
a specific, detailed and persistent memory.

## We get to this AI Memory problem

So we get to this AI Memory problem. And it was thinking about this problem that I identified
some possible solutions for what I was facing in my day to day.

I know some solutions exist. There are solutions like Mem0, there's Zep, there's Letta,
there are, well, other startups. Both open source and closed source too, trying to solve this
AI memory problem. But the Ulpia project, I believe it's going to be even more revolutionary,
let's put it that way, because I decided to build it focused not only on AI Memory, but also
focused on people who want to have things locally.

## The privacy of that data

Because that's where another point comes in. Beyond the quality of the documentation and of
the records we need to keep when we're developing software, there's also the privacy of it.
The privacy of that data, the way you store that sensitive information, business information
and so on. These things need special care, especially when you're dealing with things that
are a little more delicate.

And that's why I believe the future of memory for artificial intelligence shouldn't be a
memory hosted in a cloud service, but a memory you can have locally, and where you can run
both cloud agent models and local agent models.

Of course, today that's still not a reality, as a matter of infrastructure. Hardware is still
very expensive these days. So having laptops and computers with enough RAM, enough
processors, to be able to run artificial intelligence locally, still isn't very appealing, I
know that. But I believe very soon it will be.

And that's why I already decided to take the initiative to develop this system, which is
going to give you the possibility of having a local artificial intelligence memory, with
expressive functionality and very low latency, because we use Rust to do the retrieval path
for the information in those memories.

And in this case, I built Ulpia precisely so that you can do the retrieval and the storage of
those memories in a way where you don't depend on an artificial intelligence model
administering every action. That's one of the main points of Ulpia.

## The other solutions that exist on the market

The other solutions that exist on the market, they use the model to record those memories. We
can use it too. But they also use the model to do the retrieval, to read that information.
And our system doesn't need a model to do the retrieval.

We use an intelligent system for capturing information in your memory base, one that ranks
those memories in order to rank the files that are compatible with your input, with what
you're sending for your artificial intelligence to deal with. And only after we have a
ranking is it that we actually hand it to an AI model, whether local, whether cloud, well, an
external provider. Then that model starts to act on top of the information that file brings,
that is, that memory file.

## This repository is going to be open source

But anyway, that's it. Today I'm not going to be able to go into much detail, but I needed to
present the project. I'm going to leave the repository link, because this repository is going
to be open source, it's going to be public. So you can contribute to the evolution of this
project, and you can also self-host it, you can use it on your own machine and integrate it
with any kind of LLM you want. Whether local, whether cloud, whether, for example, an AI
provider, for example Anthropic, OpenAI, DeepSeek, well, any LLM model you want to use.

You can implement this system. It works over MCP locally too. So you're going to have the
system, and your agent is going to make calls over MCP and it's going to receive that
information over MCP too, to talk to the system.

And that way you're going to be able to save on reading tokens, because you're not going to
have to wait for your LLM model to read the file to find which file has the subject you're
looking for.

And you're going to be able to, obviously, like I said, handle the memory on your own. You
yourself can write information into the memory files, and you can also send an agent, a
model, a specific agent that you might have built as well. That's another thing that's going
to be left for another video, which is the format we use for building specialised agents. But
anyway, you can also put that model to record in memory whatever you need, the information
you require.

And that's it, folks. I hope you like the project. And we're open to feedback, obviously, so
you can leave comments, or open issues over on the GitHub repository, which we'll be taking
care of, and we're going to improve this, so it fulfils this purpose, which is to help make
artificial intelligence even more efficient, even more secure and with even more privacy.

And that's it, thanks.
