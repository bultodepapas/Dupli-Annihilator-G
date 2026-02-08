# Plan PM de Implementacion (Final)

## Documentos relacionados
- `README.md`
- `docs/00_INDICE_DOCUMENTACION_FINAL.md`
- `docs/01_RESUMEN_EJECUTIVO_FINAL.md`
- `docs/02_ESPECIFICACION_MOTOR_FINAL.md`
- `docs/03_ESPECIFICACION_UI_TAURI_FINAL.md`
- `docs/05_DECISIONES_PENDIENTES.md`

## 1) Objetivo del plan
Traducir la especificacion final en una ejecucion ordenada para equipo senior, con entregables claros, hitos verificables y control de riesgo tecnico.

## 2) Enfoque de gestion
- Entrega incremental por fases.
- Priorizacion de correctitud y rendimiento antes de polish visual.
- Congelamiento de contratos (config, eventos, stages) para evitar retrabajo.

## 3) Fases recomendadas

### Fase 0 - Baseline documental (corto)
Entregables:
- Documento final aprobado de motor.
- Documento final aprobado de UI/Tauri.
- Criterios QA cerrados.
- Registro de decisiones V1 cerrado (`docs/05_DECISIONES_PENDIENTES.md`).

Salida:
- Decision log inicial firmado por equipo.

### Fase 1 - Core engine listo para integracion
Entregables:
- Motor Rust funcional con modos RAM/DISK y ordenamientos definidos.
- Telemetria de progreso y stats agregadas.
- Validacion de reglas de parsing/salida.

Salida:
- Pruebas funcionales base superadas.

### Fase 2 - Shell Tauri + UX operativa
Entregables:
- Pantalla unica completa (Inputs, Processing, Export, Run).
- Integracion de commands/events con backend.
- Estado de ejecucion y cancelacion robustos.
- i18n base implementado (`en`, `zh-CN`) sin hardcode de strings.

Salida:
- Flujo end-to-end operativo.

### Fase 3 - Rendimiento UX + observabilidad
Entregables:
- Throttling backend/frontend aplicado.
- ETA aproximada habilitada.
- Telemetria estable en carga alta.

Salida:
- UI fluida en escenarios de volumen.

### Fase 4 - QA final + hardening
Entregables:
- Suite QA de aceptacion ejecutada.
- Revisión de accesibilidad minima AA.
- Revisión de seguridad por capabilities minimas.

Salida:
- Release candidate.

## 4) Priorizacion de trabajo (orden sugerido)
1. Correctitud funcional del motor.
2. Contrato de integracion estable.
3. UX operativa con progreso real.
4. Optimizacion final y hardening.

## 5) Riesgos principales y mitigacion

### Riesgo A: degradacion UI por exceso de eventos
Mitigacion:
- Limitar eventos de progreso a 4-10Hz.
- Batching de actualizaciones en frontend.

### Riesgo B: expectativas de orden en DISK mal comunicadas
Mitigacion:
- Tooltips y copy explicitos para `PreserveFirstSeen` en DISK.
- Matriz de garantias visible en documentacion interna.

### Riesgo C: ETA inestable
Mitigacion:
- Usar ETA aproximada.
- Mostrar `-` cuando no exista base confiable.

### Riesgo D: sobrecoste de GlobalPerfect en hardware lento
Mitigacion:
- FastBucketLocal como default recomendado.
- Exponer GlobalPerfect como opcion avanzada con advertencia.

### Riesgo E: retrabajo por cambios de contrato tardios
Mitigacion:
- Congelar naming de modes, ordering, events y payloads antes de desarrollo UI completo.

## 6) Definicion de Done (DoD) por release
1. Todos los MUST funcionales cumplidos.
2. Criterios QA de aceptacion aprobados.
3. Sin bloqueos criticos en cancelacion y escritura de salida.
4. UI estable bajo carga objetivo.
5. Documentacion de limits y comportamientos conocidos publicada.
6. SLOs V1 cumplidos en entorno controlado:
   - progreso entre 4 y 10Hz,
   - ETA mostrada solo cuando sea confiable,
   - memoria en RAM mode controlada (objetivo <=75% de RAM libre al inicio),
   - comportamiento estable de DISK mode con spill a disco.

## 7) Gobernanza de cambios
- Cualquier cambio a reglas de parsing, orden o contrato IPC requiere:
  - impacto tecnico,
  - impacto UX,
  - impacto QA,
  - decision registrada en changelog del producto.

## 8) Recomendacion de cierre
Mantener este set documental como fuente unica de trabajo para el equipo, y registrar ajustes posteriores como anexos versionados en lugar de editar decisiones historicas sin trazabilidad.
