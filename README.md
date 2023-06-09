# UniPac: Unicode-powered Pacman Adventure 

Pacman for the terminal. 

Has most of the game elements from the original game:
* Ghosts go through 'shuffle' and 'chase' (lightbulb) periods where they ignore/target pacman.
* Ghosts flee pacman when frightened.
* Ghosts slow down in the tunnel and when freightened.
* When eaten, ghost eyes trace path back to ghost house.
* Two 'fruit' bonuses on every level.

Pacman is rendered with ascii symbols and the ghosts with unicode emoticons - note that the 
original ghosts are not part of the unicode standard.

Controls are on the arrow keys.
```
% cargo run
```

![Game UI](https://raw.githubusercontent.com/jesper-olsen/UniPac/main/Screenshot.png) 



Credits:
* Steven Goodwin's [C version](https://github.com/MarquisdeGeek/pacman) of the game.
* Sound assets from Dale Harvey's [JS version](https://github.com/daleharvey/pacman)
* Marquee text by [ChatGPT 3.5](https://chat.openai.com/)
* [The Pac-Man Dossier](https://pacman.holenet.info)
