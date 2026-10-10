## Why

The owner saw artifacts in the height golden candidate. They are not z-fighting (nodes do not overlap and depth is reversed-Z): the contour decade was chosen per pixel from `fwidth`, and the choice flipped between neighbouring pixels at triangle and skirt boundaries, drawing speckle.

## What Changes

- Height contours: four decades (10 m to 10 km) each fade out continuously once their lines would be closer than ~8-10 px, instead of one discretely chosen decade. No spec change (REND-006 unchanged); one candidate golden changes.
