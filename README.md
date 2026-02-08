# Dupli-Annihilator-G - Documentacion Base

Este repositorio contiene la documentacion final organizada del producto, separada por dominio (producto, motor, UI y plan de implementacion), sin codigo.

## Ruta recomendada de lectura
1. `docs/00_INDICE_DOCUMENTACION_FINAL.md`
2. `docs/01_RESUMEN_EJECUTIVO_FINAL.md`
3. `docs/02_ESPECIFICACION_MOTOR_FINAL.md`
4. `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`
5. `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`
6. `docs/05_DECISIONES_PENDIENTES.md`

## Estado actual
- Documentacion separada y relacionada.
- Definiciones funcionales principales cerradas.
- Decisiones V1 cerradas y registradas.

## Decisiones V1 cerradas
1. Separador por defecto: `"\n"`.
2. `Mode=Auto`: alias explicito de `Ram` en V1.
3. `PreserveFirstSeen` en DISK: permitido con advertencia clara en UI.
4. Idiomas UI V1: ingles (`en`) y chino simplificado (`zh-CN`).
5. i18n escalable: sin hardcode de textos, todo por claves.
6. SLOs V1: UI fluida (4-10Hz), ETA aproximada cuando aplique, memoria controlada por modo y baseline de performance por dataset.

## Convencion de mantenimiento documental
- Si cambia una decision de producto, actualizar primero `docs/05_DECISIONES_PENDIENTES.md` y luego reflejar en:
  - `docs/01_RESUMEN_EJECUTIVO_FINAL.md`,
  - `docs/02_ESPECIFICACION_MOTOR_FINAL.md`,
  - `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`,
  - `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`.
