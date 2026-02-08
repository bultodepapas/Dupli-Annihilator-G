# Decisiones V1 (Cerradas)

## Documentos relacionados
- `README.md`
- `docs/00_INDICE_DOCUMENTACION_FINAL.md`
- `docs/01_RESUMEN_EJECUTIVO_FINAL.md`
- `docs/02_ESPECIFICACION_MOTOR_FINAL.md`
- `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`
- `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`

## Objetivo
Registrar las decisiones de producto cerradas para V1 y mantener consistencia entre documentacion funcional, tecnica y de ejecucion.

## D-01 Separador por defecto
- Estado: `CERRADA`
- Decision final:
  `"\n"` como separador por defecto.
- Impacto:
  salida legible para datasets, validacion QA mas simple y menor ambiguedad en ejemplos.

## D-02 Definicion de `Mode=Auto` en V1
- Estado: `CERRADA`
- Decision final:
  `Auto` se comporta como alias explicito de `Ram` en V1.
- Requisito de UI:
  tooltip obligatorio indicando que la heuristica automatica real queda para version posterior.
- Impacto:
  evita sobre-ingenieria temprana y reduce riesgo de comportamiento no determinista.

## D-03 Politica UI para `PreserveFirstSeen` en DISK
- Estado: `CERRADA`
- Decision final:
  se permite seleccion en DISK, con advertencia fuerte + tooltip visible.
- Mensaje minimo:
  en DISK no se garantiza orden global de primera aparicion en esta version.
- Impacto:
  transparencia funcional sin bloquear casos de uso avanzados.

## D-04 Idioma de interfaz V1
- Estado: `CERRADA`
- Decision final:
  soporte en V1 para ingles (`en`) y chino simplificado (`zh-CN`).
- Requisito de arquitectura:
  i18n por claves, sin textos hardcodeados en componentes.
- Requisito de escalabilidad:
  estructura preparada para agregar idiomas futuros sin refactor grande.
- Impacto:
  base internacional desde V1 con deuda tecnica de localizacion controlada.

## D-05 SLOs de rendimiento de aceptacion
- Estado: `CERRADA`
- Decision final:
  se adoptan SLOs iniciales de V1 para QA en entorno controlado.
- SLOs V1 recomendados:
  1. UI: no congelamiento percibido durante ejecucion; actualizacion de progreso entre 4 y 10 Hz.
  2. ETA: mostrar ETA aproximada cuando exista base confiable; en caso contrario mostrar `-`.
  3. RAM mode: limite objetivo de memoria pico <= 75% de RAM libre al inicio del job.
  4. DISK mode: uso de memoria acotado y estable, priorizando spill a disco.
  5. Performance base: registrar throughput y tiempos por dataset de referencia para controlar regresiones por release.
- Nota:
  los valores numericos finos por tamano de dataset se calibran en el primer ciclo de benchmarks del equipo.

## Politica de mantenimiento
Si se cambia una decision cerrada:
1. actualizar esta misma entrada con motivo y fecha,
2. sincronizar `docs/01_RESUMEN_EJECUTIVO_FINAL.md`,
3. sincronizar `docs/02_ESPECIFICACION_MOTOR_FINAL.md` y/o `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`,
4. actualizar impacto de plan en `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`.
