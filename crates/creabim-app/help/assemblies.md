# Layered assemblies

Real walls, floors and roofs are made of layers: brick, cavity, insulation, block, plaster. An **assembly** is a named list of those layers, each with its material, thickness and function, kept in the project and shared by every element that uses it. Plans and sections hatch each layer in its material, and the 3D view shows one solid per layer.

## The Assemblies window

Open it with *Materials › Assemblies…*, or the **⋯** on the browser's *Assemblies* group.

- Assemblies are grouped as walls, floors and roofs.
- The layer table runs from the exterior face to the interior face: function, material and thickness, with buttons to reorder and remove layers.
- The preview shows the layers hatched, with the **core** (the structural layers) bracketed.
- Create, duplicate and delete assemblies, see how many elements use one, select them, or assign it to the selection.

![The Assemblies window](img/assemblies.jpg)

*Materials › Add starter assemblies* adds an exterior brick cavity wall, an interior block wall, a concrete floor and a tiled pitched roof.

## Using one

Select walls, slabs or roofs, then pick an assembly with *Materials › Assign assembly* or the *Assembly* picker in *Properties*. The thickness comes from the layers. *None* goes back to a single material.

A wall's layers start at its exterior side; [flip](walls#inside-and-outside-flip) it to swap sides.

## Joins

Layered walls join layer by layer: L corners are mitred, and in a T the core runs on to the other wall's core.

## Doors and windows in layered walls

By default every layer stops square at a door or window. To have layers turn into the reveal and run to the frame, as plaster and render do on site, open the wall's assembly and set **Wrap at openings** to *Exterior*, *Interior* or *Both*: the faces of the wall the layers turn in from.

- Each layer has a **Wraps** tick. Finishes wrap unless you untick them; any other layer wraps when you tick it.
- A layer that wraps lines the jambs, the head and the sill from its own place in the wall to the face of the frame. Where several wrap, the outermost lies against the opening and the next one behind it.
- The layers they cover stop short by the thickness of the returns, so the opening keeps its size. A bare hole has no frame: its returns stop at the core.
- One door or window can differ from its wall: set **Layers wrap** in its *Properties*.

Plans, sections and the 3D view all show the wrapped layers, and the layer solids written to IFC include them. The take-off still measures each layer without its returns.

Where nothing wraps, the 3D view lines the reveals with the wall's finishes: the outer finish on the outside part of the reveal, the inner finish on the inside part. So the insulation and membranes don't show where the opening cuts through them. Those linings are only for the 3D view; drawings and exports show the layers as they are.
