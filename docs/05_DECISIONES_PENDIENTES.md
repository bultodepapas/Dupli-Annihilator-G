# Decisiones Pendientes (Cierre V1)

## Documentos relacionados
- `README.md`
- `docs/00_INDICE_DOCUMENTACION_FINAL.md`
- `docs/01_RESUMEN_EJECUTIVO_FINAL.md`
- `docs/02_ESPECIFICACION_MOTOR_FINAL.md`
- `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`
- `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`

## Objetivo
Centralizar las decisiones que aun requieren validacion de producto/arquitectura para evitar ambiguedades durante implementacion.

## D-01 Separador por defecto
- Contexto:
  El separador es configurable, pero falta fijar default de producto.
- Opciones:
  - `"\n"` (dataset-friendly, legible por linea),
  - `" "` (salida lineal continua).
- Recomendacion:
  `"\n"`.
- Impacto:
  UX inicial, ejemplos, QA y documentacion.

## D-02 Definicion de `Mode=Auto` en V1
- Contexto:
  En el origen aparece como heuristica futura y tambien como opcion visible.
- Opciones:
  - Auto como alias de RAM en V1,
  - Auto con heuristica minima por metadatos de entrada,
  - no exponer Auto hasta V1.1.
- Recomendacion:
  Alias de RAM en V1 con tooltip explicito.
- Impacto:
  Complejidad tecnica, expectativa de usuario y soporte.

## D-03 Politica UI para `PreserveFirstSeen` en DISK
- Contexto:
  En DISK no se garantiza orden global de primera aparicion en esta version.
- Opciones:
  - permitir seleccion con advertencia clara,
  - bloquear seleccion en DISK,
  - remapear automaticamente a `FastBucketLocal`.
- Recomendacion:
  Permitir con advertencia fuerte + tooltip.
- Impacto:
  Correctitud percibida, transparencia y riesgo de malinterpretacion.

## D-04 Idioma de interfaz V1
- Contexto:
  La documentacion base esta en espanol y parte del copy de UI esta en ingles.
- Opciones:
  - Espanol,
  - Ingles,
  - bilingue.
- Recomendacion:
  Ingles tecnico en UI + glosario interno en espanol (si el equipo es mixto).
- Impacto:
  Consistencia de copy, soporte y onboarding.

## D-05 SLOs de rendimiento de aceptacion
- Contexto:
  Hay direccion tecnica de performance, pero faltan umbrales formales de aprobado/rechazado.
- Definir:
  - tiempo maximo objetivo por tamano de input,
  - limite de memoria aceptable por modo,
  - frecuencia minima de actualizacion de progreso sin congelamiento UI.
- Recomendacion:
  Fijar SLOs por entorno de prueba controlado antes de QA final.
- Impacto:
  QA objetivo, release readiness y comparabilidad de builds.

## Registro de decisiones
Cuando se cierre cada decision:
1. marcarla como `CERRADA` en este archivo,
2. reflejar el cambio en `docs/01_RESUMEN_EJECUTIVO_FINAL.md`,
3. reflejar el cambio en `docs/02_ESPECIFICACION_MOTOR_FINAL.md` o `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md` segun aplique,
4. sincronizar fechas y version en `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`.
