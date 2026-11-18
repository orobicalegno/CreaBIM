# Rooms

A **room** is a space on a level: the study, the hall, the kitchen. It has a name and a number, and it knows its own **area**, **perimeter** and **height**. Those come from the walls round it, so they update as soon as a wall moves. Rooms carry the data a room schedule lists: floor, wall and ceiling finishes, occupancy and department.

## Placing rooms

1. Pick the **Room** tool (in *Create › Building elements*, after Object).
2. Click inside walls. The room fills the space to the walls' **inner faces**, takes the next number on its level (0.01, 0.02, … on the ground floor, 1.01, … on the first) and gets a tag in the level's plans.
3. Type its name in *Properties*, and its finishes and occupancy if you want them in the schedule.

The tool also has the slab tool's ways of drawing, picked with `1`–`4` or in the tool options:

- **Fill room (3)** is the click inside walls described above. The room keeps only its point; its outline is found again from the walls every time it is drawn, so it **follows the walls**.
- **Polygon (1)**, **Rectangle (2)** and **From sketch (4)** draw the room's outline yourself. Use these for a terrace, a loggia or a balcony, where no walls close the space off. A drawn room keeps its outline and doesn't follow the walls; *Follow the walls instead* in *Properties* turns it back into one that does.

Clicking where a room already is selects that room instead of making a second one.

## Room separators

An open-plan space has no wall between the kitchen and the dining room. Draw a **room separator** where the line between them goes: pick the **Room separator** tool and click its two ends. Rooms on either side of it are then measured apart, and each one follows the separator when you move it. Separators show as thin lines in plans and aren't printed in sections or 3D.

## What a room measures

- **Area**: to the inner faces of the walls round it, less any shaft through its floor (a stair well, a service riser). It is written in the project's units, in m² or ft².
- **Perimeter**: the length of its outline.
- **Height**: up to the ceiling over it if it has one, else up to the level above. Tick *Own height* in *Properties* to set one of its own.

If the walls round a room's point stop closing it off (you deleted one, or moved it too far), the room says **Not enclosed** in its tag, in *Properties* and in the schedule, until the walls close it again or you draw its outline.

## Room tags

Room tags show in plans, and are left out of detail and callout views unless the view's *Show room tags* is on.

Every new room gets a **tag** at its point in each plan of its level. It reads the room's name in capitals, then its number and its area. Drag the tag to move it; it stays with its room. Pick a room tag and tick, in *Properties › Shows*, the fields it writes:

- *Name*, *Number*, *Area*, *Perimeter*, *Height*, *Level*
- *Floor finish*, *Wall finish*, *Ceiling finish*, *Occupancy*, *Department*

A field the room has no value for is left out. The tag's text style is the *Room name* style when the project has one; change it like any other tag's. See [Tags](tags) and [Annotation styles](styles).

The **Tag rooms** button in a plan's view options tags every room on that plan that has no tag there yet. The **Tag** tool tags a room or a ceiling too.

## Room notes from older projects

Projects made before rooms existed wrote room names and areas as **notes** ("LIVING / 37.2 m²"). *Create › Rooms from room notes* turns each such note into a room with a tag in its place, wherever walls close it off. The notes that stay (for an open terrace, or two labels in one space) are listed in the status bar: draw those rooms, or add room separators and run it again. The command-line tool does the same with `creabim rooms-from-notes <model> <out>`.

## Room colours

Tick **Colour rooms** in a plan's or a section's *Properties* (or **Room colours** in a plan's view options) to fill each room with a colour of its own. A room has the same colour in every view, and a section shows the rooms it cuts as bands up to their height.

## The room schedule

*File › New schedule › Rooms* makes a schedule of every room, with its number, name, level, area, perimeter, height, floor finish and occupancy. Rooms are grouped by level, each level with its number of rooms and its total area, and the last line totals the whole building. See [Schedules](schedules). `creabim schedule <model> rooms` prints it as CSV.

## IFC

Rooms are exported as **IfcSpace**: the number as its *Name*, the name as its *LongName*, its outline as a prism up to its height, and its area, perimeter and height in *Qto_SpaceBaseQuantities*. Finishes and occupancy go in *Pset_SpaceCoveringRequirements* and *Pset_SpaceOccupancyRequirements*. Each wall round it is linked by an **IfcRelSpaceBoundary** (a separator by a virtual one), and the ceilings over it by **IfcRelCoversSpaces**.

When an IFC file is opened, each IfcSpace becomes a room again. A room Volumen wrote follows its walls again; a space from another program keeps the outline of its body. See [IFC](ifc).

**See also:** [Ceilings](ceilings), [Furniture and fixtures](furniture)
