# Views

In Volumen a **view** is a way of looking at the model, kept in the project: a floor plan, a section, an elevation, a 3D view or a schedule. Views are drawn from the model each time, so they never go out of date. They save, undo and update with it.

## Kinds of views

- **Floor plans**: the building cut at a height above a level (the *Cut height*, usually about 1.2 m) and seen from above. Cut walls are drawn heavy and hatched; doors show their swing. See [How plans are drawn](views#how-plans-are-drawn).
- **Sections** and **elevations**: see [Sections and elevations](sections).
- **3D views**: saved, parallel 3D views, such as *3D View South-East*, to place on sheets. Set the angle with *Altitude*, and pick a *Style*.
- **Schedules**: tables of elements. See [Schedules](schedules).

## Opening a view

- Click it in the project browser: they are grouped as *Floor plans*, *3D views*, *Sections*, *Elevations* and *Schedules*.
- Or pick it in an area's header: the view-type menu, then the view's name.
- `PageUp` and `PageDown` step to the previous or next view of the same kind.

The view opens in the area you last worked in. Several areas can show views side by side; see [View areas](interface#view-areas). You can draw and select in any of them with every tool.

## The view's settings

The header of the area showing a view holds its settings, and *Properties* (the *View* tab, with nothing selected) has the rest:

- *Cut height* (plans), *Depth* (sections and elevations) or *Altitude* (3D views);
- *Style* (sections, elevations and 3D views): *Hidden line*, *Shaded with edges*, or *Wireframe*, which draws every edge, the hidden ones too. Sheets show the view the same way. Shafts are drawn dashed;
- *Hidden* and *In colour*: elements hidden inside others drawn dashed, and tinted in their colour (see [Visibility](visibility#hidden-elements));
- *X-ray*: see through walls and slabs (see [Visibility](visibility#x-ray));
- *Drawing style* (plans and sections): how what is cut is drawn (see [Graphic styles](graphic-styles#drawing-styles));
- *Temporary dimensions* (plans): on or off;
- *Tag openings*: tag every door and window that has no tag;
- *Underlay* (plans): another level shown faintly underneath;
- *Visibility*: what this view leaves out.

With nothing selected, *Properties* shows the same and more, such as the view's scale.

## How plans are drawn

Plans follow the usual conventions of permit drawings:

- **Line weights** step down from heavy to light: cut walls, columns and footings are heaviest, then what is seen below the cut (slab edges, the stair), then symbols (door leaves and swings, window frames), then furniture and fixtures, then hatching. Lines overhead (beams, wall cupboards) are light and dashed. The weights suit the view's scale: heavier at 1:50, lighter at 1:100. They come from the project's [style pack](graphic-styles).
- **Below the cut**, furniture, fittings and imported meshes get a light tint of their material's colour under their lines.
- **Joins**: cut walls and footings of the same material that touch or overlap merge into one outline, with no lines inside.
- **Stairs**: on its own level, a stair is drawn up to a break line where the plan cuts it. On the level above, the stair is seen through its well, with *DN*, and the well is not crossed over it. A void's cross runs corner to corner in each part of the opening, so an L-shaped well gets a cross in each leg. On the stair's own level the well above is a dashed outline.
- **Roof plans** show an arrow down each sloping face with its pitch (*35°*, or *1:60* for shallow slopes).
- **Labels**: room names, notes, tags and plain labels are moved off grid lines, section lines, walls and each other to the nearest clear spot. A label moved far gets a leader back to where it was. Grid and section bubbles and dimension text stay where they are. On the command line, `creabim label-clashes <model>` lists any labels that still overlap.

### Plan graphics

A plan's *Properties* has a *Graphics* group:

- *Tint what lies below the cut*, its *Strength*, and *In grey, not material colours*.
- *View depth*: how far below its floor the plan looks through holes in the floor, such as a stair well. Between the floor and the cut is *below* the cut, drawn in projection. Under the floor, down to the view depth, is *beyond*. By default the plan looks down to the level below.
- *Line weights from the style pack*: untick it to set each pen's width in millimetres on paper (cut, projection, symbols, furniture and fixtures, hatching, hidden) for this view only. Sheets, PDF and SVG use the same weights.
- The pen, colour, pattern and hatch of each category are **object styles** of the project's style pack, and a view can override them: see [Graphic styles](graphic-styles).
- *View range*: what levels a plan draws, and the roof or site plan seen from above: see [View range](view-range).

### North arrow, grid and axes

- **North arrow**: *⋯ › North arrow* in a plan's header places a small, plain north arrow in the middle of what the plan shows. Drag it where it goes. It points to the project's true north (*Project information*), unless it is given an angle of its own in *Properties*, where its size (paper millimetres) is set too. It shows on the sheets the plan is placed on.
- **Grid behind drawings** (`F7`) and **X and Y axes**: on and off in *⋯* in a view's header, or in *View › Every view*.

### The crosshairs

In plans and sections the pointer is AutoCAD's crosshairs: two lines right across the view, with the square pickbox while selecting. Near a snap they lock onto it. *Settings › Interface › 2D cursor* turns them off or sets how long they are (100% runs right across the view).

## Ordering the browser

Drag a view up or down in its group. The group then keeps your order (it says *Your order*); its sort button puts it back in automatic order (plans by level).

## Cameras

A **camera** is a perspective view, as an Archicad camera: where you stand, what you look at, and how wide you see. It is kept under *3D views* in the project browser, drawn with hidden lines like any 3D view, and goes on sheets, into PDF and DXF.

- **Place one in a plan**: pick the **Camera** tool (in the Views group), set the *Eye height* above the plan's level and the *Angle* in the top bar, then click where the camera stands and click what it looks at.
- **Save the 3D window as one**: *View › Camera › Save as a camera* keeps what the 3D window sees now, from where it is, as a camera. Walk to the spot with [Explore on foot](navigate#exploring-on-foot) (`F5`), then save it.
- **In plans** a camera shows on the floor it stands on: its body, the edges of what it sees out to its target, and a cross on the target. Select it to drag the camera or its target with their grips, or move and rotate it like anything else.
- **In Properties**: the eye's and the target's place and height (above ±0), the *Angle* (60° is a usual lens, 90° a wide one) and the picture's *Proportion* (1.5 is 3:2, 1.78 is 16:9), its style and its scale. **Look through it in the 3D window** puts the 3D window's camera there.
- **A selected camera and the 3D window follow each other.** Select a camera and the 3D window looks through it, with *Looking through* shown at its foot. Whatever you change in its *Properties*, drag with its grips or undo shows in the 3D window at once. Move in the 3D window (orbit, pan, zoom, or walk with `F5`) and the camera is moved with you, saved each time a move ends; each move is one undo step. Select something else, or nothing, and the 3D window is your own again.

The camera's picture is framed: what is outside its angle is left out, and so is anything closer than 10 cm.

