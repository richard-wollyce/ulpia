# The demo fleet

Three tiny agents, checked into the repository so the numbers on the front page are an
experiment you can run rather than a claim you have to believe:

```
kb route "quem decide se um deploy pode ir pra producao" examples/demo
kb eval examples/demo/gold.tsv examples/demo
```

`zed` is a software architect, `steve` a marketing analyst, `yaron` a nutrition
assistant, each holding a few textbook notes. `gold.tsv` is the answer key: ten
questions the fleet should answer, three it should refuse. Everything here is invented
demo content; the real fleet this repository's authors run is private, which is the
product working as designed.
