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
- Pendientes puntuales listados para decision de producto/equipo.

## Decisiones que faltan para cerrar alcance V1
1. Definir separador por defecto del producto:
   - opcion A: `"\n"` (recomendada para datasets),
   - opcion B: `" "` (alineada a lectura continua).
2. Definir comportamiento final de `Mode=Auto` en V1:
   - opcion A: alias explicito de `Ram`,
   - opcion B: heuristica minima por tamano de entrada,
   - opcion C: postergar heuristica y ocultar `Auto` en UI inicial.
3. Definir politica de `PreserveFirstSeen` en DISK en UI:
   - opcion A: permitir con advertencia fuerte,
   - opcion B: deshabilitar seleccion en DISK,
   - opcion C: remapear automatico a opcion soportada.
4. Definir idioma de interfaz inicial:
   - opcion A: Espanol,
   - opcion B: Ingles,
   - opcion C: bilingue desde V1.
5. Definir objetivos de rendimiento de aceptacion (SLO internos):
   - tiempo objetivo por tamano de entrada,
   - limite maximo de uso de RAM por modo,
   - fluidez minima UI durante ejecucion.

## Convencion de mantenimiento documental
- Si cambia una decision de producto, actualizar primero `docs/05_DECISIONES_PENDIENTES.md` y luego reflejar en:
  - `docs/01_RESUMEN_EJECUTIVO_FINAL.md`,
  - `docs/02_ESPECIFICACION_MOTOR_FINAL.md`,
  - `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`,
  - `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`.

