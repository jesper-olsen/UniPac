# UniPac: Unicode-powered Pacman Adventure 

Pacman for the terminal. 

Has most of the game elements from the original game:
* Ghosts go through 'shuffle' and 'chase' (💡) periods where they ignore/target pacman.
* Ghosts flee pacman when frightened.
* Ghosts slow down in the tunnel and when freightened.
* When eaten, ghost eyes trace a path back to the ghost house.
* Two 'fruit' bonuses on every level.
* Cornering.

Mazes:
* Shortened pacman maze (24 rows).
* Regular pacman maze (29 rows).
* 4 Ms Pacman mazes (31 rows).

Pacman is animated with ascii symbols and the ghosts with unicode codepoints (Pinky 👺, Blinky 👹, Inky 👻, Clyde 🎃); 
 Unicode has symbols for most of the fruit bonuses (🍒,🍓,🍑,🍎,🍇,🚀,🔔,🔑), but not for the ghosts themselves.

Controls are on the arrow keys.
```
% cargo run --release
```

![Game UI](https://raw.githubusercontent.com/jesper-olsen/UniPac/main/Screenshot.png) 



Credits:
* Steven Goodwin's [C version](https://github.com/MarquisdeGeek/pacman) of the game.
* Sound assets from Dale Harvey's [JS version](https://github.com/daleharvey/pacman)
* Marquee text by [ChatGPT](https://chat.openai.com/)
* [The Pac-Man Dossier](https://pacman.holenet.info)
* [Ms Pac-Man Walkthrough](https://strategywiki.org/wiki/Ms._Pac-Man/Walkthrough)
