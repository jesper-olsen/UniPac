# UniPac demo - render with:  vhs demo.tape
#
# Build the game first, so compile time stays out of the recording:
#     cargo build --release
#
# Pacman moves one tile per frame (about 150 ms) and keeps going until a wall stops him, so each
# key below is pressed shortly after he has reached the end of a corridor. The Sleeps are the
# measured times for this route. The ghosts move randomly, so roughly one run in four ends with
# pacman caught early - if so, just render again.

Output demo.gif

Set FontSize 20
Set Width 1400
Set Height 900
Set Padding 20
Set Framerate 20
Set TypingSpeed 10ms

Type "./target/release/unipac"
Sleep 500ms
Enter

Sleep 2.54s
# pacman has run left along row 18 and stopped at the wall: go down
Down

Sleep 0.32s
# left along row 20
Left

Sleep 0.86s
# down to the bottom row
Down

Sleep 0.32s
# run right along the whole bottom row, eating 25 dots
Right

Sleep 3.97s
# up the right edge
Up

Sleep 0.29s
# left along row 20
Left

Sleep 0.29s
# up into the pill alcove
Up

Sleep 0.29s
# right: eat the power pill - the ghosts turn blue
Right

Sleep 0.44s
# back out of the alcove
Left

Sleep 0.20s
# down
Down

Sleep 0.29s
# left
Left

Sleep 0.44s
# up the right-hand corridor
Up

# keep recording while the ghosts are frightened
Sleep 3.30s
