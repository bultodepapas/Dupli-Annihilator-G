# Especificacion UI/Tauri v2 (Final, sin codigo)

## Documentos relacionados
- `README.md`
- `docs/00_INDICE_DOCUMENTACION_FINAL.md`
- `docs/01_RESUMEN_EJECUTIVO_FINAL.md`
- `docs/02_ESPECIFICACION_MOTOR_FINAL.md`
- `docs/04_PLAN_PM_IMPLEMENTACION_FINAL.md`
- `docs/05_DECISIONES_PENDIENTES.md`

## 1) Objetivo de la aplicacion desktop
Una unica pantalla para:
1. seleccionar multiples archivos de entrada,
2. configurar modo, orden y separador de salida,
3. elegir archivo final de exportacion,
4. ejecutar, monitorear y cancelar el job sin bloquear UI.

## 2) Stack recomendado (nivel equipo senior)
- Tauri v2 para contenedor desktop.
- Frontend React + TypeScript.
- Sistema visual con Tailwind y componentes headless.
- Motor pesado en Rust; frontend solo orquesta y visualiza.

## 3) Requisitos funcionales MUST

### 3.1 Inputs
- Seleccion de N archivos (boton + drag and drop).
- Lista con nombre, tamano, ruta truncada y remove individual.
- Accion `Clear all`.

### 3.2 Configuracion de procesamiento
- Selector de `Mode`: `AUTO`, `RAM`, `DISK`.
- Regla V1 para `AUTO`: se comporta como `RAM` y debe mostrarse tooltip explicativo.
- Selector de `Output Ordering`:
  - `PreserveFirstSeen` (default),
  - `Alphabetical`,
  - `UnorderedFast` (advanced).
- Si `DISK + Alphabetical`, mostrar subopciones:
  - `Fast (Recommended)` = FastBucketLocal,
  - `Global Perfect (Slower)` = GlobalPerfect.
- Si `DISK + PreserveFirstSeen`, mostrar advertencia fuerte:
  no se garantiza orden global de primera aparicion en esta version.

### 3.3 Separador de salida
- Debe aceptar string arbitrario.
- Debe ofrecer presets comunes.
- Debe permitir custom separator.
- Debe incluir toggle `Interpret escapes` (ON por defecto).
- Debe mostrar preview visible del resultado.

### 3.4 Output file
- Seleccion de ruta y nombre por Save dialog.
- Validacion de salida no vacia.
- Confirmacion de overwrite cuando corresponda.

### 3.5 Ejecucion y cancelacion
- Boton principal con estados:
  - `RUN`,
  - `CANCEL` (durante ejecucion),
  - `RUN AGAIN` (finalizado),
  - `RETRY` (error).
- Estados operativos visibles:
  - `Idle`, `Running`, `Finalizing`, `Done`, `Error`, `Canceled`.

## 4) Progreso, telemetria y ETA

### 4.1 Principio de rendimiento
- Sin streaming de logs por token.
- Solo telemetria agregada y eventos throttled.

### 4.2 Progreso visual
- Barra de progreso destacada.
- Determinate cuando haya base confiable.
- Indeterminate cuando no exista base confiable.
- Stage actual y detail line obligatorios.

### 4.3 Metricas live
- `tokens_seen`
- `unique_tokens`
- `duplicates`
- `throughput (tokens/sec)` suavizado
- `elapsed`
- `ETA (approx)` o `-` cuando no aplique

### 4.4 Stages de referencia en UI
- RAM:
  - ScanningInputs,
  - Tokenizing,
  - Deduplicating,
  - Sorting (si aplica),
  - WritingOutput,
  - Finalizing.
- DISK fast:
  - PartitioningBuckets,
  - ReducingBuckets,
  - WritingOutput,
  - Finalizing.
- DISK global perfect:
  - GeneratingRuns,
  - MergingRuns,
  - WritingOutput,
  - Finalizing.

## 5) Contrato de integracion Frontend <-> Rust

### 5.1 Commands
- `start_job(config)` retorna identificador de trabajo.
- `cancel_job(job_id)` solicita cancelacion.
- `get_app_info()` opcional.

### 5.2 Events
- `job://started`
- `job://progress`
- `job://done`
- `job://error`
- `job://canceled`

### 5.3 Campos minimos esperados en progreso
- jobId,
- stage,
- progress01 best-effort,
- filesDone/filesTotal,
- tokensSeen/uniqueTokens/duplicates,
- throughput,
- elapsed,
- eta,
- detail.

## 6) Seguridad en Tauri v2
- Modelo de capacidades minimas por ventana.
- Lectura/escritura pesada solo en Rust.
- Frontend con acceso a FS solo para dialogos y metadatos necesarios.

## 7) Performance UI obligatoria
1. Throttling backend->frontend: 4-10 actualizaciones por segundo maximo.
2. Throttling/batching en frontend para evitar re-render excesivo.
3. Evitar miles de nodos en pantalla (logs infinitos o tablas gigantes).
4. Virtualizar lista solo cuando volumen de items lo justifique.

## 8) Sistema visual final ("Neon Lab")

### 8.1 Direccion visual
- Dark-first con acentos neon controlados.
- Estetica de instrumento cientifico, no arcade.
- Jerarquia limpia, ruido visual minimo.

### 8.2 Paleta base consolidada
- Fondos: `#05070D`, `#0B1020`, `#0F1730`.
- Texto: `#E6F0FF` y variantes atenuadas.
- Acentos: cyan, magenta, lime, amber, red.
- Borde y glow discretos para foco y estado activo.

### 8.3 Layout final de pantalla unica
- Header: marca + modo actual.
- Panel Inputs.
- Panel Processing.
- Panel Export.
- Footer de telemetria en vivo.

### 8.4 Microinteracciones
- Hover/focus claros y breves.
- Running con animacion minima y estable.
- Success y error sin efectos distractores.

## 9) Localizacion e internacionalizacion (MUST)
- Idiomas V1 obligatorios:
  - ingles (`en`),
  - chino simplificado (`zh-CN`).
- No se permite hardcodear textos en componentes.
- Todo copy visible debe salir de claves i18n.
- La estructura i18n debe permitir agregar nuevos idiomas sin refactor grande.

## 10) Accesibilidad MUST
- Contraste minimo AA.
- Estados no solo por color.
- Navegacion por teclado.
- Respeto a `reduce motion` del sistema.

## 11) Manejo de errores
- Error panel con mensaje humano corto.
- Detalle tecnico colapsable.
- Boton para copiar reporte de debug.
- Accion `Retry`.

## 12) Criterios de aceptacion QA
1. Flujo completo RAM con salida valida.
2. Flujo DISK + Alphabetical Fast con stages correctos.
3. Flujo DISK + Alphabetical GlobalPerfect con stages correctos.
4. Interpretacion correcta de escapes del separador.
5. Cancelacion efectiva sin bloqueo de UI.
6. UI fluida bajo carga con actualizaciones <=10Hz.
7. Cumplimiento de contraste y foco visible.
8. Cambio de idioma `en` <-> `zh-CN` sin reinicio de app.
