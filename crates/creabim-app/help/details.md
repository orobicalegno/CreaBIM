# Details

A **detail** is a larger-scale drawing of one part of the building, such as the base of a wall, a window head or the eaves. It is a view of its own, called out by a box on the plan or section it details. You draw over it with **detail components**: insulation, membranes, flashings and other parts that the model doesn't hold.

## Callouts

1. Open a plan or section.
2. Pick the **Callout** tool (in the Views group).
3. Pick the scale of the detail in the tool options: 1:2, 1:5, 1:10 or 1:20.
4. Click two corners of a box around the part to detail.

This makes a new view, named "Detail 1", "Detail 2" and so on, and opens it. The detail shows what its parent shows, cropped to the box and drawn at its own scale. On the parent, the box has a bubble with the detail's number. The number is the first word of the detail's name that has a digit in it: "Detail 3 Eaves" shows 3. If you rename the view, the bubble changes with it.

![Section C-C of the starter house, with the boxes of its three details](img/details.jpg)

Details are listed in their own **Details** group of the project browser. A detail works like any other view: you can place it on a [sheet](sheets), set its [visibility](visibility) and [crop](crop-boxes) it. If you delete the detail, its box disappears from the parent. If you delete the parent, the detail stays and keeps showing the same part.

## Detail components

1. Open the view to draw on, usually a detail.
2. Pick the **Detail component** tool (in the Annotation group).
3. In the tool options, pick the kind of component and its width.
4. Click its points. To finish, click the last point again or press Enter.

The kinds are:

- **Batt insulation**: a quilt between two lines, with one loop per width along its length.
- **Membrane**: a heavy line, for a damp-proof course, a vapour control layer or a breather membrane.
- **Flashing**: a thin solid strip, for a metal flashing or a drip.
- **Blocking**: a piece of timber in section, drawn as a box with a cross.
- **Repeating detail**: a row of units with joints between them, such as bricks or blocks.
- **Region**: a filled area that hides what is under it, hatched with a [material](materials) hatch. To close it, click the first point again.
- **Break line**: the zigzag where a drawing stops. It takes two points.

A detail component belongs to the view it is drawn in and shows in no other view. Detail components are drafting, not building parts: they don't appear in schedules, and IFC export leaves them out. To name them, add [notes](notes) with leaders.

## Linework

The same tool draws plain lines, as a CAD program does, for anything the components don't cover. Its second group in the tool options has the shapes:

- **Line**: click its points; one stretch or many. Click the last point again, or press Enter, to finish.
- **Closed line**: the same, closed back to its start. Click the first point again to close it.
- **Rectangle**: two opposite corners.
- **Polygon**: its centre, then a corner. Set the number of *Sides* in the tool options.
- **Circle**: its centre, then a point on it.
- **Arc**: its start, a point it passes through, then its end.
- **Ellipse**: its centre, the end of one axis, then a point that gives the other axis its length.
- **Spline**: a smooth curve through the points you click.

Pick the **pen** beside the shapes: heavy, medium, light, fine or dashed. These are the drawing's own line weights, so linework prints like the rest of the view. Change a line's pen, or open and close it, in *Properties*.

A rectangle or a polygon is kept as a closed line once drawn, so it can turn and be reshaped corner by corner. Linework moves, rotates, mirrors, copies and deletes like a detail component, and like one it shows only in the view it was drawn in.

## The details in the starter house

The [starter house](starter-house) has a sheet **A501 Details**. It shows section C-C through the south wall at 1:25, with three callouts at 1:10: the wall base, the window head and sill, and the eaves.
