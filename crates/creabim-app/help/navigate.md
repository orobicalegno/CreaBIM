# Moving around the views

## In the 3D view

The same mouse in every view (*Settings › Navigation › Mouse*; *Classic* keeps a left drag for orbiting):

| Do this | To |
| --- | --- |
| Click with the left button | Select (`Shift` adds) |
| Drag with the left button on empty space | Select with a box |
| Drag with the left button on something selected | Move it |
| Drag with the middle button | Orbit round the model |
| `Alt` + drag with the left button | Orbit, with a mouse that has no middle button |
| Drag with the right button, or `Shift` + middle | Pan |
| Click with the right button without moving | The menu for what is under the pointer |
| Roll the wheel | Zoom toward the pointer |
| `Ctrl` + drag with the middle button | Zoom, as in Blender: drag up to zoom in, down to zoom out |
| `Home` | Frame everything |
| `.` | Frame the selection |

The buttons at the top right of the 3D view do the same, and more: zoom in and out, orbit round the selection, perspective or orthographic, the ground grid, [X-ray](visibility#x-ray), **Show** (what the 3D view shows), the **render** menu (*Shaded*, *Shaded with edges*, *Hidden line* or *Wireframe*, also in *View › Render*; wireframe shows shafts and what is inside walls) and **View** (look from the top, the front, a side or a corner).

**Orbiting turns about the model under the pointer**: press where you want to turn about and drag, and that point stays where it is while a mark shows it, in every mouse scheme. On empty sky it turns about the middle of the view. *Orbit around the selection* (in *View › Camera*) turns about the selection instead. The **wheel zooms toward the point of the model under the pointer**, which stays put, and never stops short of it; a **pan holds what you grabbed under the pointer**, near or far.

**Archicad users**: pick *Archicad* in *Settings › Navigation › Mouse*: a middle drag pans, `Shift` + middle drag orbits, and a double-click with the middle button frames everything, as in Archicad's 3D window. Left click still selects, and a right click opens the menu.

### Exploring on foot

`F5` (or *View › Camera › Explore on foot*) puts you in the model, as Archicad's Explore mode does. `W` `A` `S` `D` or the arrow keys walk forward, left, back and right along the ground, `Q` and `E` go down and up, and `Shift` makes it a run. A middle or right drag turns your head; the wheel steps ahead. `F5` or `Esc` goes back to orbiting. While exploring, those keys walk and start no tools.

### The view cube

The cube in the top right corner of the 3D view turns the view for you:

- Click a **face** (*Top*, *Front*, *Right*…), an **edge** or a **corner** to look from there. The view turns smoothly and the status bar says where you are looking from, for example *Looking from the top, front, left (south-west)*.
- Click **N**, **E**, **S** or **W** on the ring under it to look from that side.
- The buttons beside it: drag the **magnifier** up or down to zoom, and the **four arrows** to pan; the **house** frames everything from the three-quarter view, as `Home` does; the two **turn** buttons turn the view a quarter round, from a corner to the next corner; the **grid** switches between perspective (a grid going away) and orthographic (a square grid).
- Drag the cube to orbit.

*Front* is the south face: the cube is named from north, which is up in plans, so *Front* shows what the south elevation shows.

## In plans, sections and sheets

| Do this | To |
| --- | --- |
| Drag with the left button on empty paper | Select with a box |
| Drag with the right or middle button | Pan |
| Roll the wheel | Zoom |
| `Ctrl` + drag with the middle button | Zoom, as in the 3D view |
| The buttons at the top right | Zoom in and out (click a step, or drag up and down), fit, frame the selection, and drag the hand to pan |
| *Zoom to scale* (*⋯ More*) | 1 mm on paper is 1 mm on screen |

## Layout

- The window's middle is split into [view areas](interface#view-areas); **Workspace** at the top right picks a layout of them (*Model*, *Drawing*, *Document*, *Sheets*, or your own), and `Ctrl+PgUp` / `Ctrl+PgDn` step through them.
- `Ctrl+Space` shows the area under the pointer alone, and again brings the others back.
- Each area pans and zooms on its own; *Pan and zoom with the other area of this view* in its *⋯ More* menu links two areas of one view.
- In the *View* menu, under *2D view*: *In its own window* moves the drawing to a window of its own, for a second screen; *Overlaid in the 3D workspace* shows it as an inset over the 3D view.
- *View › Show the 2D view in 3D space* draws the plan or section in the 3D view, where it cuts.

## Touchpads

A two-finger scroll pans plans, sheets and the 3D view; a pinch zooms. A mouse wheel still zooms.

## The ground grid

The 3D view shows a grid on the working level. Turn it off with the grid button in its navigation bar, and pick its spacing in *View › Ground grid*, including any spacing you type. The model's X axis runs through it in red and Y in green (*Settings › Viewport*); Z shows in blue only while a move keeps to it.

Plans and sections have a **Grid** button in their header too, with faint red and green lines through the origin.
