# SPIKE-14: egui panel over the lit frame (adapter: Vulkan NVIDIA GeForce RTX 3080 Ti Laptop GPU (DiscreteGpu), key vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu)
Lit viewer frame 1920x1080 (573 nodes, atmosphere), panel: 5 sliders, checkbox, combo box, text field, scrolling list of 100 items. Times in ms, median / p90 over 300 frames.

| DPI scale | primitives | vertices | egui run + tessellate (CPU) | UI pass record + submit + GPU wait | pixels outside the panel that differ from the blit alone |
|---|---|---|---|---|---|
| 100 % | 5 | 2730 | 0.175 / 0.186 | 0.116 / 0.168 | 0 |
| 150 % | 5 | 2186 | 0.179 / 0.185 | 0.123 / 0.189 | 0 |
