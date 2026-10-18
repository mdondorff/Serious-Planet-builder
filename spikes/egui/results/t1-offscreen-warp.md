# SPIKE-14: egui panel over the lit frame (adapter: Dx12 Microsoft Basic Render Driver (Cpu), key dx12-microsoft-basic-render-driver)
Lit viewer frame 1920x1080 (573 nodes, no atmosphere), panel: 5 sliders, checkbox, combo box, text field, scrolling list of 100 items. Times in ms, median / p90 over 120 frames.

| DPI scale | primitives | vertices | egui run + tessellate (CPU) | UI pass record + submit + GPU wait | pixels outside the panel that differ from the blit alone |
|---|---|---|---|---|---|
| 100 % | 5 | 2730 | 0.160 / 0.182 | 17.203 / 17.586 | 0 |
| 150 % | 5 | 2186 | 0.153 / 0.159 | 22.929 / 23.823 | 0 |
