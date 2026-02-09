# Update Strategy Implementation Plan (Desktop Tauri)

## 1) Objetivo
Definir una estrategia robusta de actualizaciones para que el usuario final reciba nuevas versiones con friccion minima, y para que el equipo publique releases con menos errores operativos.

Objetivos concretos:
- reducir pasos manuales para actualizar;
- eliminar inconsistencias entre `main`, tags y release assets;
- mantener seguridad (integridad/autenticidad) del update;
- tener rollback operativo rapido.

No objetivos (v1 del plan):
- canales complejos multitenant (enterprise-hosted por cliente),
- delta updates avanzadas por plataforma,
- rollout porcentual por cohortes.

## 2) Estado Actual (As-Is)
- Se publican instaladores via GitHub Actions (`desktop-release.yml`) al pushear tags `v*`.
- La app no tiene flujo in-app de actualizacion (no hay `tauri-plugin-updater` en runtime).
- Build actual de release usa `--no-sign`.
- Riesgo observado: se puede taggear desde una rama distinta a `main`, y el release queda publicado aunque `main` no refleje ese estado.

Consecuencia:
- el usuario tiene que actualizar manualmente desde GitHub Releases;
- hay riesgo de confusion operacional y soporte (version visible vs branch base);
- no hay mecanismo app-driven para descubrir/instalar updates.

## 3) Opciones Evaluadas

### Opcion A - Mantener modelo manual actual
Descripcion:
- usuario descarga instalador nuevo desde Releases.

Ventajas:
- cero cambios de arquitectura.

Desventajas:
- mala UX de actualizacion;
- alta friccion;
- mayor carga de soporte.

Riesgo:
- medio/alto (adopcion lenta de parches).

### Opcion B - Check in-app + abrir pagina de release (sin auto-instalacion)
Descripcion:
- la app verifica version disponible y ofrece boton "Descargar update".
- redirige a release URL en navegador.

Ventajas:
- mejora rapida, sin pipeline criptografico completo.

Desventajas:
- sigue siendo manual;
- no elimina friccion final de instalacion.

Riesgo:
- bajo.

### Opcion C - Auto-update completo con Tauri Updater (recomendada)
Descripcion:
- app detecta update, descarga, instala y solicita reinicio.
- pipeline publica artifacts + metadata de update firmada.

Ventajas:
- mejor UX final;
- ciclo de parche mas rapido;
- menos tickets operativos de "como actualizo".

Desventajas:
- requiere disciplina de firma y pipeline.

Riesgo:
- medio (setup inicial), bajo luego de estabilizar.

### Opcion D - Servicio propio de updates (API dedicada)
Descripcion:
- endpoint propio para manifests, canales, rollout y politicas.

Ventajas:
- control total de estrategia de despliegue.

Desventajas:
- mayor costo de plataforma y operacion.

Riesgo:
- medio/alto en complejidad.

## 4) Decision Estrategica
Adoptar estrategia escalonada:

1. Fase 1 (quick win): Opcion B
- check de version in-app + CTA de descarga.
- endurecer release process y version gates.

2. Fase 2 (target): Opcion C
- auto-update completo con firma y metadata oficial.

Razon:
- minimiza riesgo de rollout;
- entrega valor temprano;
- prepara base para auto-update sin bloquearse por detalles de firma el primer sprint.

## 5) Aristas y Consecuencias a cubrir

### 5.1 Seguridad / Integridad
- Sin firma criptografica no se debe auto-instalar.
- Secrets de firma deben estar solo en GitHub Environments protegidos.
- Necesario rotacion de llaves y plan de revocacion.

### 5.2 Operacion / Release Hygiene
- Tag solo permitido desde `main`.
- Workflow debe fallar si versiones no coinciden:
  - `apps/desktop/package.json`
  - `apps/desktop/src-tauri/Cargo.toml`
  - `apps/desktop/src-tauri/tauri.conf.json`
  - crates versionadas del workspace.

### 5.3 UX / Producto
- No interrumpir jobs activos: update pendiente pero no instalable hasta terminar run.
- Estado visible del update:
  - idle
  - checking
  - available
  - downloading
  - ready_to_restart
  - failed

### 5.4 Compatibilidad
- Definir politica semver:
  - patch/minor auto-update permitido
  - major opcionalmente manual (decision de producto).
- Manejar caso usuario sin permisos de instalacion o entorno corporativo bloqueado.

### 5.5 Soporte
- incluir `appVersion`, `backendVersion`, y `updateChannel` en reportes de diagnostico.
- mostrar errores accionables (network, firma, permisos, storage).

## 6) Arquitectura Propuesta (Target)

### 6.1 Frontend (React)
- Servicio de update central (`useUpdater` o `updateService.ts`).
- Trigger:
  - check en startup con delay corto,
  - check manual por boton.
- UI:
  - banner no intrusivo;
  - modal opcional para "instalar ahora" cuando termina descarga.

### 6.2 Tauri Runtime (Rust)
- Integrar plugin updater y permisos/capabilities asociados.
- Exponer comandos minimos opcionales para telemetria de update.
- Bloquear instalacion si `runStatus == running`.

### 6.3 CI/CD
- release workflow con gates estrictos.
- publicar metadata requerida por updater.
- publicar artifacts por OS + checksums + firma.

## 7) Pseudocodigo (adelanta implementacion)

### 7.1 Estado de update en frontend
```ts
type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; notes?: string }
  | { kind: "downloading"; progressPct: number }
  | { kind: "ready_to_restart"; version: string }
  | { kind: "failed"; reason: string };

function useUpdater(runStatus: RunStatus) {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  async function checkForUpdates(manual = false) {
    setState({ kind: "checking" });
    try {
      const update = await updater.check(); // plugin-updater
      if (!update) {
        setState({ kind: "idle" });
        return;
      }
      setState({ kind: "available", version: update.version, notes: update.body });
    } catch (e) {
      setState({ kind: "failed", reason: String(e) });
    }
  }

  async function installUpdate() {
    if (runStatus === "running") return; // guard hard
    try {
      setState({ kind: "downloading", progressPct: 0 });
      await updater.downloadAndInstall((event) => {
        setState({ kind: "downloading", progressPct: event.percent ?? 0 });
      });
      setState({ kind: "ready_to_restart", version: "pending" });
    } catch (e) {
      setState({ kind: "failed", reason: String(e) });
    }
  }

  async function restartApp() {
    await relaunch(); // plugin-process
  }

  return { state, checkForUpdates, installUpdate, restartApp };
}
```

### 7.2 Integracion en `main.tsx`
```ts
// On startup:
useEffect(() => {
  const id = setTimeout(() => void checkForUpdates(false), 1500);
  return () => clearTimeout(id);
}, []);

// UI:
// - boton "CHECK UPDATES"
// - banner cuando state.kind === "available"
// - boton "UPDATE NOW" bloqueado si runStatus === "running"
// - boton "RESTART TO APPLY" cuando ready_to_restart
```

### 7.3 Rust/Tauri bootstrap
```rust
fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .manage(AppState { backend: BackendService::new() })
    .invoke_handler(tauri::generate_handler![
      start_job, cancel_job, get_app_info, get_runtime_state, path_exists,
      default_output_path, next_events, open_output, open_output_folder, export_summary_json
    ])
    .run(tauri::generate_context!())
    .expect("failed to run tauri app");
}
```

### 7.4 Release workflow gates
```yaml
job: verify-version-coherence
  steps:
    - read versions from package.json, tauri.conf.json, Cargo.toml(s)
    - fail if mismatch
    - fail if tag != v{version}

job: verify-tag-source
  steps:
    - ensure tagged commit is reachable from origin/main
    - fail otherwise

job: build-and-sign
  steps:
    - build artifacts
    - sign updater artifacts (secret key)
    - generate updater metadata

job: publish
  steps:
    - upload assets + updater metadata
    - create/update release notes
```

### 7.5 Script de release unificado
```bash
# pseudo-cli: npm run release:minor
1) verify clean workspace
2) bump versions in all manifests
3) update lockfiles
4) run tests + frontend build
5) commit "chore(release): vX.Y.Z"
6) tag vX.Y.Z
7) push branch + tag
```

## 8) Plan de Implementacion por Fases

### Fase 0 - Preparacion (1-2 dias)
Deliverables:
- checklist de secretos (firma updater),
- decision de canal inicial (`stable`),
- contrato UX para update states.

Gate:
- aprobacion de seguridad y operaciones.

### Fase 1 - Harden release process (1 dia)
Deliverables:
- `verify-version-coherence` en CI,
- regla "tag debe salir de main",
- release script unificado.

Gate:
- imposible publicar release inconsistente.

### Fase 2 - In-app check (Opcion B) (1-2 dias)
Deliverables:
- boton `Check updates`,
- banner de nueva version con link a release.

Gate:
- deteccion de nueva version funcional en QA.

### Fase 3 - Auto-update core (Opcion C) (2-4 dias)
Deliverables:
- plugin updater + process integrados,
- install/restart flow,
- bloqueo durante job running.

Gate:
- update end-to-end exitoso en Windows y macOS.

### Fase 4 - Stabilizacion (1-2 dias)
Deliverables:
- logs de observabilidad y errores tipados,
- fallback UX robusto.

Gate:
- 0 blockers P1/P0 en pruebas de regresion.

## 9) Matriz de Riesgos y Mitigaciones

Riesgo: metadata de update invalida o incompleta.
- Mitigacion: job CI de validacion de metadata antes de publicar.

Riesgo: keys de firma mal configuradas.
- Mitigacion: entorno de staging con release de prueba firmado.

Riesgo: update durante procesamiento activo.
- Mitigacion: guard de estado + UI que difiere instalacion.

Riesgo: mismatch de version entre manifests.
- Mitigacion: gate obligatorio + script de release unico.

Riesgo: usuario corporativo sin permisos.
- Mitigacion: fallback a descarga manual + mensaje explicito.

## 10) Testing Strategy

### Unit
- parser de estado update,
- mapeo de errores a mensajes UX,
- guard de `runStatus === running`.

### Integration
- flujo check -> available -> install -> restart (mock de updater),
- flujo de error de red/firma/permisos.

### E2E Release
- dry-run en tag de staging,
- verificacion de assets + metadata + install real en VM limpia.

## 11) Definition of Done
1. Update check visible y funcional en UI.
2. Auto-install y restart funcional en Windows + macOS.
3. CI impide releases con versiones inconsistentes.
4. Tag fuera de `main` bloqueado.
5. Playbook de rollback documentado.

## 12) Open Decisions (pendientes)
1. Politica de major updates: auto o manual.
2. Canal beta/publico desde el inicio o solo stable.
3. Frecuencia default de update check (startup-only vs periodic).
4. Nivel de telemetria de update permitido por privacidad.

## 13) Checklist de Ejecucion Inmediata
- [ ] Definir llave de firma y secrets en GitHub Environment.
- [ ] Agregar gate de coherencia de versiones en CI.
- [ ] Agregar gate de "tag reachable from main".
- [ ] Implementar `useUpdater` + UI base.
- [ ] Integrar plugins updater/process en Tauri.
- [ ] Ejecutar release de prueba `v1.3.1-rc1`.
- [ ] Validar update real desde `v1.3.0` a `v1.3.1`.
