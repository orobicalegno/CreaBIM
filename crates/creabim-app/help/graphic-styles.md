# Graphic styles

How every drawing looks is data in the project, not built into Volumen: pen widths for each scale, how each category of element draws (its **object style**), the hatch patterns of cut faces, and **view templates**. All of it is kept together as a **style pack**, a `.json` file an office can keep its drawing standard in. Load another pack and every view and sheet is restyled.

## Style packs

Open *File › Graphic styles…*.

- The *Style pack* list shows the packs in the project. The one ticked is the default: the project draws with it.
- *Add a shipped pack*: Volumen comes with two, *Standard* (its usual look: black linework, grey poché, the usual hatches) and *Light linework* (fine pens, pale blue-grey poché, furniture in grey, sections fading with depth, sun shadows on plans from above).
- *Load pack…* reads a `.json` pack and makes it the default. A pack of the same name is replaced. The pack's text styles, title blocks and mark numbering replace the project's of the same names.
- *Save pack as…* writes the pack to a file, to use in another project or to share.

A project without a pack draws with the *Standard* pack. Loading a pack is one undo step.

On the command line, `creabim apply-style <project> <pack> --pdf sheets.pdf` loads a pack (a file, or *Standard* or *Light linework*) into a project and exports its sheets. `creabim style-packs <folder>` writes the shipped packs as files to start your own from.

## Drawing styles

Each plan and section has a **Drawing style**: how what is cut is drawn. Pick it in the view's header, or under *Graphics* in its *Properties*. It is kept with the view, and so in a view template, and the screen, sheets and PDFs match.

| Style | What is cut |
| --- | --- |
| **Technical** (the default) | Hatched by material, as the graphic styles set them |
| **Poché** | Walls and slabs in solid ink, no hatch |
| **Colour and hatch** | Each material in its own colour, layer by layer, with its hatch over it: brick red, insulation yellow |
| **Material colour** | Each material in its own colour, layer by layer, no hatch |
| **Colour wash** | A soft wash of the materials' colours, for presentation |
| **Grey wash** | An even light grey, no hatch |
| **Clean** | Outlines only: no hatches or fills |

In the colour styles, roofs seen from above under a plan's cut are washed too, the lowest first, so a higher roof covers a lower one.

## Object styles

The *Object styles* page has a row for each category: walls, slabs, roofs, columns, doors, windows, furniture and so on.

- *Projection*: the weight of its lines seen below the cut or in elevation. *Cut*: the weight of its outline where the view cuts it. Each is *As drawn* (Volumen's pen), one of the pens (so it follows the scale), or a width in millimetres.
- *Colour*: a line colour of its own.
- *Line*: *Solid*, *Dashed*, *Dotted*, *Centre* or *Dash dot*.
- *Cut fill*: the colour behind its cut hatch. *Cut pattern*: a pattern for its cut faces; *Material's* keeps each material's own.

The pack's file can also set a style for one category in one material, a surface fill and pattern (a roof seen from above, or a hard landscape area), the draw order and transparency.

A hard landscape area keeps the paving pattern set on it. The Site style's surface fill goes under every area. Its surface pattern is drawn only on areas whose own pattern is *None*.

## Pens

The *Pens* page lists the pen widths, in millimetres on paper, for each range of scales. A drawing takes the first table its scale fits: with *Up to 1:75* and *Up to 1:100000*, a 1:50 plan takes the first and a 1:100 plan the second.

## Patterns

A hatch pattern is one or more families of parallel lines (an angle, a spacing, an offset, a stagger and dashes, where a dash of 0 is a dot), or a small tile repeated, over a background colour.

- *on paper (mm)*: the pattern is the same size at any scale, like fixed-size dots.
- *true size (m)*: the pattern is drawn to the model's size, like brick coursing or tiles.

The list starts empty: it holds the pack's own patterns. To get some:

- **From the library**: Volumen's architectural patterns, each with its preview: *Cemento*, *Calcestruzzo armato*, *Sabbia*, *Ghiaia*, *Terreno*, *Mattoni*, *Mattoni sezione*, *Blocchi*, *Pietra*, *Legno*, *Piastrelle*, *Cartongesso*, *Acciaio*, *Isolante rigido*, *Diagonali*, *Incrociato*, *Erba*, *Acqua*.
- **Import .pat…**: every pattern of an AutoCAD `.pat` file, such as `acadiso.pat` from your AutoCAD (in its *Support* folder), or any other. Sizes are read as millimetres on paper.
- **Copy a built-in**: one of the patterns materials use by default, to change it.
- **New pattern**: one line family to start from.

In a family's *Dashes*, type dash, gap, dash, gap… separated by spaces (`2 1` dashed, `4 1 0 1` dash and dot, `0 1` dots; empty is solid). *Stagger* slides each line's dashes along from the one before.

A material's cut faces take the pattern its *Cut hatch* names (*Brick*, *Concrete*…). A pack pattern with the same name replaces that built-in for every material using it. A material can also take any pattern of the pack: pick it in *Pattern*, under *Cut hatch* in the material's editor.

## View templates

A view template is a named bundle: scale, style, the categories its views leave out, pens of its own, distance fading and sun shadows. Every view that uses it changes when it changes.

- Give a view a template under *Template and overrides* in its *Properties*, or select views in the project browser and use *Give it to the N selected views* on the *View templates* page.
- *Views without a template take* names the template a view of each kind gets from the pack when it has none of its own.
- Views name their template, so loading another pack with templates of the same names restyles them.

## Tags and marks

The *Tags and marks* page sets how every room tag and view marker in the project is drawn. Each choice is a row with a picture: rest the pointer on one to see it on the drawings, move off to go back, and click to keep it.

- **Room tags:** *Stacked* (name, area, then the number small), *Number badge* (the number in a small outline on top), *Boxed* (a box with the name over the number and area) or *Inline* (number and area on one line).
- **View markers:** *Classic*, *Minimal*, *Bold* or *Plain*, for section marks, elevation marks and callouts together.
- **Your own markers:** *+ New style from …* opens the marker designer. Pick a head, fill, text, pointer, line, callout edge and colour, watch the preview, name it and *Save as style*. It is listed under *Your styles*, with *Edit…* and *Delete*. Your styles are part of the style pack, so *Save pack as…* and *Load pack…* carry them to other projects.

## Overrides

A view can change how a category or one element draws in it only, over the object styles.

- In a view's *Properties*, under *Template and overrides*, *Add a category…* adds an override: *Halftone*, *No hatch*, a *Weight*, a *Line* pattern or a *Colour*.
- For one element, select it with the view open: *In <view>* in its *Properties* has the same fields, for example the boundary wall dashed in the site plan only.

## Depth cues

Under *Template and overrides*:

- *Fade with distance*: in sections, elevations and plans from above, lines and fills further from the cut or the viewer draw lighter.
- *Sun shadows*: in plans from above, each element casts a shadow on the ground, *Toward* a direction, with a *Length* for each metre of height and a *Strength*.

Cut elements always take the heavy cut pen.
