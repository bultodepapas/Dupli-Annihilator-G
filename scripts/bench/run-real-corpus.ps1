[CmdletBinding()]
param(
    [ValidateSet("all", "smoke-real", "bench-real-core", "bench-real-rich")]
    [string]$Suite = "all",

    [string]$OutputDir = "target/real-corpus-bench",

    [string]$CliPath,

    [switch]$SkipBuild,

    [switch]$ListScenarios,

    [switch]$ValidateCorpus,

    [switch]$RequireCorpus
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

function Get-DefaultCliPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $binaryName = if ($IsWindows) { "dedupe_cli.exe" } else { "dedupe_cli" }
    return Join-Path $RepoRoot (Join-Path "target\release" $binaryName)
}

function New-RunSpec {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string]$Mode,

        [Parameter(Mandatory = $true)]
        [string]$Ordering,

        [string]$DiskAlphabeticalMode = "fast-bucket-local",

        [int]$DiskBuckets = 256,

        [string]$DiskRunSize = "256MB"
    )

    return [pscustomobject]@{
        Name                 = $Name
        Mode                 = $Mode
        Ordering             = $Ordering
        DiskAlphabeticalMode = $DiskAlphabeticalMode
        DiskBuckets          = $DiskBuckets
        DiskRunSize          = $DiskRunSize
    }
}

function Get-ScenarioDefinitions {
    $ramPreserve = New-RunSpec -Name "ram_preserve" -Mode "ram" -Ordering "preserve-first-seen"
    $ramUnordered = New-RunSpec -Name "ram_unordered" -Mode "ram" -Ordering "unordered-fast"
    $diskPreserve = New-RunSpec -Name "disk_preserve" -Mode "disk" -Ordering "preserve-first-seen"
    $diskAlphaGlobal = New-RunSpec -Name "disk_alpha_global" -Mode "disk" -Ordering "alphabetical" -DiskAlphabeticalMode "global-perfect"
    $autoPreserve = New-RunSpec -Name "auto_preserve" -Mode "auto" -Ordering "preserve-first-seen"

    return @(
        [pscustomobject]@{
            Name        = "small_dictionary"
            Suite       = "smoke-real"
            Description = "70k-line dense wordlist smoke corpus."
            InputKeys   = @("spanish_txt")
            Runs        = @($ramPreserve, $ramUnordered, $diskPreserve)
        }
        [pscustomobject]@{
            Name        = "small_epub"
            Suite       = "smoke-real"
            Description = "Small EPUB rich-input smoke corpus."
            InputKeys   = @("metamorfosis_epub")
            Runs        = @($ramPreserve, $autoPreserve)
        }
        [pscustomobject]@{
            Name        = "single_huge_unique"
            Suite       = "bench-real-core"
            Description = "Single large text corpus that is almost entirely unique."
            InputKeys   = @("test2_csv")
            Runs        = @($ramPreserve, $ramUnordered, $diskPreserve, $autoPreserve)
        }
        [pscustomobject]@{
            Name        = "two_huge_partial_overlap"
            Suite       = "bench-real-core"
            Description = "Two large text corpora with partial overlap."
            InputKeys   = @("test1_csv", "test3_csv")
            Runs        = @($ramPreserve, $ramUnordered, $diskPreserve, $diskAlphaGlobal, $autoPreserve)
        }
        [pscustomobject]@{
            Name        = "rich_duplicate_pair"
            Suite       = "bench-real-rich"
            Description = "Two byte-identical PDFs to isolate extraction + overlap-100% dedupe."
            InputKeys   = @("biology_pdf_a", "biology_pdf_b")
            Runs        = @($ramPreserve, $diskPreserve, $autoPreserve)
        }
        [pscustomobject]@{
            Name        = "rich_mixed"
            Suite       = "bench-real-rich"
            Description = "Mixed rich-input workload with PDF + large EPUB."
            InputKeys   = @("pablo_pdf", "dune_epub")
            Runs        = @($ramPreserve, $diskPreserve, $autoPreserve)
        }
    )
}

function Resolve-RealCorpus {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,

        [switch]$RequirePresent
    )

    $testfilesDir = Join-Path $RepoRoot "testfiles"
    if (-not (Test-Path -LiteralPath $testfilesDir -PathType Container)) {
        if ($RequirePresent) {
            throw "Missing real benchmark corpus directory: $testfilesDir"
        }
        return $null
    }

    $biologyPdfs = @(Get-ChildItem -LiteralPath $testfilesDir -File | Where-Object {
            $_.Name -like "*44638b230c0a6734a3c32b66b2ba0ed0*.pdf"
        } | Sort-Object Name)
    if ($biologyPdfs.Count -ne 2) {
        throw "Expected 2 Biology PDFs in testfiles/, found $($biologyPdfs.Count)."
    }

    $pabloPdf = @(Get-ChildItem -LiteralPath $testfilesDir -File | Where-Object {
            $_.Name -like "*a3b22e2d4c54ee67642136dd94f3f5ca*.pdf"
        })
    if ($pabloPdf.Count -ne 1) {
        throw "Expected 1 Pablo Escobar PDF in testfiles/, found $($pabloPdf.Count)."
    }

    $requiredNamedFiles = @{
        test1_csv         = "Test1.csv"
        test2_csv         = "Test2.csv"
        test3_csv         = "Test3.csv"
        spanish_txt       = "spanish.txt"
        dune_epub         = "Dune.epub"
        metamorfosis_epub = "La metamorfosis.epub"
    }

    $resolved = [ordered]@{}
    foreach ($entry in $requiredNamedFiles.GetEnumerator()) {
        $path = Join-Path $testfilesDir $entry.Value
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing required corpus file: $path"
        }
        $resolved[$entry.Key] = (Resolve-Path -LiteralPath $path).Path
    }

    $resolved["biology_pdf_a"] = $biologyPdfs[0].FullName
    $resolved["biology_pdf_b"] = $biologyPdfs[1].FullName
    $resolved["pablo_pdf"] = $pabloPdf[0].FullName
    $resolved["testfiles_dir"] = $testfilesDir

    return $resolved
}

function Validate-RealCorpus {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$Corpus
    )

    $biologyAHash = (Get-FileHash -LiteralPath $Corpus["biology_pdf_a"] -Algorithm SHA256).Hash
    $biologyBHash = (Get-FileHash -LiteralPath $Corpus["biology_pdf_b"] -Algorithm SHA256).Hash
    if ($biologyAHash -ne $biologyBHash) {
        throw "Biology PDF pair is no longer byte-identical."
    }

    $rows = @(
        [pscustomobject]@{ Key = "test1_csv"; Name = [IO.Path]::GetFileName($Corpus["test1_csv"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["test1_csv"]).Length / 1MB, 2) }
        [pscustomobject]@{ Key = "test2_csv"; Name = [IO.Path]::GetFileName($Corpus["test2_csv"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["test2_csv"]).Length / 1MB, 2) }
        [pscustomobject]@{ Key = "test3_csv"; Name = [IO.Path]::GetFileName($Corpus["test3_csv"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["test3_csv"]).Length / 1MB, 2) }
        [pscustomobject]@{ Key = "spanish_txt"; Name = [IO.Path]::GetFileName($Corpus["spanish_txt"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["spanish_txt"]).Length / 1MB, 3) }
        [pscustomobject]@{ Key = "biology_pdf_pair"; Name = "2 x Biology PDF (SHA256 matched)"; SizeMB = [math]::Round(((Get-Item -LiteralPath $Corpus["biology_pdf_a"]).Length * 2) / 1MB, 2) }
        [pscustomobject]@{ Key = "pablo_pdf"; Name = [IO.Path]::GetFileName($Corpus["pablo_pdf"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["pablo_pdf"]).Length / 1MB, 2) }
        [pscustomobject]@{ Key = "dune_epub"; Name = [IO.Path]::GetFileName($Corpus["dune_epub"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["dune_epub"]).Length / 1MB, 2) }
        [pscustomobject]@{ Key = "metamorfosis_epub"; Name = [IO.Path]::GetFileName($Corpus["metamorfosis_epub"]); SizeMB = [math]::Round((Get-Item -LiteralPath $Corpus["metamorfosis_epub"]).Length / 1MB, 2) }
    )

    Write-Output "real corpus: OK"
    $rows | Format-Table -AutoSize | Out-String | Write-Output
}

function Ensure-CliBinary {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,

        [string]$RequestedCliPath,

        [switch]$SkipCliBuild
    )

    $resolvedPath = if ($RequestedCliPath) {
        if ([IO.Path]::IsPathRooted($RequestedCliPath)) {
            $RequestedCliPath
        } else {
            Join-Path $RepoRoot $RequestedCliPath
        }
    } else {
        Get-DefaultCliPath -RepoRoot $RepoRoot
    }

    if (Test-Path -LiteralPath $resolvedPath -PathType Leaf) {
        return (Resolve-Path -LiteralPath $resolvedPath).Path
    }

    if ($SkipCliBuild) {
        throw "CLI binary not found and -SkipBuild was specified: $resolvedPath"
    }

    Push-Location $RepoRoot
    try {
        cargo build -p dedupe_cli --release
    }
    finally {
        Pop-Location
    }

    if (-not (Test-Path -LiteralPath $resolvedPath -PathType Leaf)) {
        throw "CLI binary still missing after build: $resolvedPath"
    }

    return (Resolve-Path -LiteralPath $resolvedPath).Path
}

function Get-SelectedScenarios {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$AllScenarios,

        [Parameter(Mandatory = $true)]
        [string]$SelectedSuite
    )

    if ($SelectedSuite -eq "all") {
        return $AllScenarios
    }

    return @($AllScenarios | Where-Object { $_.Suite -eq $SelectedSuite })
}

function Show-ScenarioList {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Scenarios
    )

    $rows = foreach ($scenario in $Scenarios) {
        [pscustomobject]@{
            Suite    = $scenario.Suite
            Scenario = $scenario.Name
            Inputs   = ($scenario.InputKeys -join ",")
            Runs     = (($scenario.Runs | ForEach-Object { $_.Name }) -join ",")
        }
    }

    $rows | Sort-Object Suite, Scenario | Format-Table -AutoSize | Out-String | Write-Output
}

function Convert-SizeToBytes {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Size
    )

    $text = $Size.Trim().ToUpperInvariant()
    if ($text -match "^(\d+)(B|KB|MB|GB)?$") {
        $value = [int64]$matches[1]
        $unit = if ($matches[2]) { $matches[2] } else { "B" }
        switch ($unit) {
            "B" { return $value }
            "KB" { return $value * 1KB }
            "MB" { return $value * 1MB }
            "GB" { return $value * 1GB }
        }
    }
    throw "Unsupported disk-run-size literal: $Size"
}

function Get-StageDurationValue {
    param(
        $StageDurations,
        [Parameter(Mandatory = $true)]
        [string]$StageName
    )

    if ($null -eq $StageDurations) {
        return $null
    }

    if ($StageDurations.PSObject.Properties.Name -contains $StageName) {
        return [int64]$StageDurations.$StageName
    }

    return $null
}

function Invoke-BenchmarkRun {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CliBinary,

        [Parameter(Mandatory = $true)]
        [object]$Scenario,

        [Parameter(Mandatory = $true)]
        [object]$Run,

        [Parameter(Mandatory = $true)]
        [string[]]$Inputs,

        [Parameter(Mandatory = $true)]
        [string]$OutputRoot
    )

    $scenarioDir = Join-Path $OutputRoot $Scenario.Name
    $outputDir = Join-Path $scenarioDir "outputs"
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

    $outputPath = Join-Path $outputDir ($Run.Name + ".txt")
    $args = New-Object System.Collections.Generic.List[string]
    foreach ($inputPath in $Inputs) {
        [void]$args.Add("--input")
        [void]$args.Add($inputPath)
    }
    [void]$args.Add("--output")
    [void]$args.Add($outputPath)
    [void]$args.Add("--overwrite")
    [void]$args.Add("--quiet")
    [void]$args.Add("--benchmark-json")
    [void]$args.Add("--mode")
    [void]$args.Add($Run.Mode)
    [void]$args.Add("--ordering")
    [void]$args.Add($Run.Ordering)
    [void]$args.Add("--disk-buckets")
    [void]$args.Add([string]$Run.DiskBuckets)
    [void]$args.Add("--disk-run-size")
    [void]$args.Add($Run.DiskRunSize)
    if ($Run.Mode -eq "disk" -or $Run.Name -like "disk_*") {
        [void]$args.Add("--disk-alphabetical-mode")
        [void]$args.Add($Run.DiskAlphabeticalMode)
    }

    $rawLines = & $CliBinary @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Benchmark run failed for $($Scenario.Name)/$($Run.Name): $($rawLines -join [Environment]::NewLine)"
    }

    $jsonLine = @($rawLines | Where-Object { $_ -and $_.TrimStart().StartsWith("{") }) | Select-Object -Last 1
    if (-not $jsonLine) {
        throw "No benchmark summary JSON received for $($Scenario.Name)/$($Run.Name). Raw output: $($rawLines -join [Environment]::NewLine)"
    }

    $summary = $jsonLine | ConvertFrom-Json
    $extractMs = Get-StageDurationValue -StageDurations $summary.stage_durations_ms -StageName "ExtractingText"
    $inputNames = @($Inputs | ForEach-Object { [IO.Path]::GetFileName($_) })

    return [pscustomobject]@{
        suite                          = $Scenario.Suite
        scenario                       = $Scenario.Name
        description                    = $Scenario.Description
        variant                        = $Run.Name
        input_names                    = ($inputNames -join ";")
        input_count                    = $Inputs.Count
        input_bytes_total              = [int64]$summary.input_bytes_total
        tokens_seen                    = [int64]$summary.tokens_seen
        unique_tokens                  = [int64]$summary.unique_tokens
        duplicates                     = [int64]$summary.duplicates
        filtered_by_length             = [int64]$summary.filtered_by_length
        elapsed_ms                     = [int64]$summary.elapsed_ms
        avg_throughput_tps             = [int64]$summary.avg_throughput_tps
        output_bytes                   = [int64]$summary.output_bytes
        mode_requested                 = $Run.Mode
        mode_effective                 = [string]$summary.mode_effective
        ordering_requested             = $Run.Ordering.Replace("-", "_")
        ordering_effective             = [string]$summary.ordering
        disk_alphabetical_mode_request = $Run.DiskAlphabeticalMode.Replace("-", "_")
        disk_alphabetical_mode_effect  = if ($null -ne $summary.disk_alphabetical_mode) { [string]$summary.disk_alphabetical_mode } else { "" }
        disk_buckets                   = [int]$Run.DiskBuckets
        disk_run_bytes                 = Convert-SizeToBytes -Size $Run.DiskRunSize
        extract_ms                     = $extractMs
        core_pipeline_ms               = if ($null -ne $extractMs) { [int64]$summary.elapsed_ms - [int64]$extractMs } else { [int64]$summary.elapsed_ms }
        warnings                       = if ($summary.warnings) { ($summary.warnings -join " | ") } else { "" }
        status                         = [string]$summary.status
        output_path                    = [string]$summary.output_path
        stage_durations_json           = if ($null -ne $summary.stage_durations_ms) { $summary.stage_durations_ms | ConvertTo-Json -Compress } else { "" }
        started_at                     = [string]$summary.started_at
        finished_at                    = [string]$summary.finished_at
    }
}

function Test-CountParity {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Results,

        [Parameter(Mandatory = $true)]
        [string[]]$Variants
    )

    $present = @($Results | Where-Object { $Variants -contains $_.variant })
    if ($present.Count -lt 2) {
        return [pscustomobject]@{
            passed = $true
            detail = "Not enough variants present to compare."
        }
    }

    $baseline = $present[0]
    foreach ($candidate in $present | Select-Object -Skip 1) {
        if ($candidate.tokens_seen -ne $baseline.tokens_seen -or
            $candidate.unique_tokens -ne $baseline.unique_tokens -or
            $candidate.duplicates -ne $baseline.duplicates) {
            return [pscustomobject]@{
                passed = $false
                detail = "Count mismatch between $($baseline.variant) and $($candidate.variant)."
            }
        }
    }

    return [pscustomobject]@{
        passed = $true
        detail = "All compared variants reported identical counts."
    }
}

function New-ValidationRecord {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Scenario,

        [Parameter(Mandatory = $true)]
        [string]$Check,

        [Parameter(Mandatory = $true)]
        [bool]$Passed,

        [Parameter(Mandatory = $true)]
        [string]$Detail
    )

    return [pscustomobject]@{
        scenario = $Scenario
        check    = $Check
        passed   = $Passed
        detail   = $Detail
    }
}

function Get-ProjectVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $cliCargoToml = Join-Path $RepoRoot "apps/cli/Cargo.toml"
    $content = Get-Content -LiteralPath $cliCargoToml -Raw
    $match = [regex]::Match($content, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $match.Success) {
        return "unknown"
    }

    return $match.Groups[1].Value
}

function Get-GitRevision {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    try {
        Push-Location $RepoRoot
        try {
            $rev = git rev-parse --short HEAD 2>$null
            if ($LASTEXITCODE -eq 0 -and $rev) {
                return ($rev | Select-Object -First 1).Trim()
            }
        }
        finally {
            Pop-Location
        }
    }
    catch {
    }

    return "unknown"
}

function Get-ValidationRecords {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Results
    )

    $records = @()
    $byScenario = $Results | Group-Object scenario

    foreach ($group in $byScenario) {
        $scenarioName = $group.Name
        $scenarioResults = @($group.Group)

        switch ($scenarioName) {
            "single_huge_unique" {
                foreach ($result in $scenarioResults) {
                    $passed = ($result.unique_tokens -eq $result.tokens_seen) -and ($result.duplicates -eq 0)
                    $detail = "variant=$($result.variant) unique=$($result.unique_tokens) tokens=$($result.tokens_seen) duplicates=$($result.duplicates)"
                    $records += New-ValidationRecord -Scenario $scenarioName -Check "unique_equals_tokens_seen" -Passed $passed -Detail $detail
                }
            }
            "two_huge_partial_overlap" {
                $parity = Test-CountParity -Results $scenarioResults -Variants @("ram_preserve", "disk_preserve", "auto_preserve")
                $records += New-ValidationRecord -Scenario $scenarioName -Check "ram_disk_auto_count_parity" -Passed $parity.passed -Detail $parity.detail
            }
            "small_dictionary" {
                foreach ($result in $scenarioResults) {
                    $passed = ($result.unique_tokens -gt 0) -and ($result.output_bytes -gt 0)
                    $detail = "variant=$($result.variant) unique=$($result.unique_tokens) output_bytes=$($result.output_bytes)"
                    $records += New-ValidationRecord -Scenario $scenarioName -Check "non_empty_output" -Passed $passed -Detail $detail
                }
            }
            "small_epub" {
                foreach ($result in $scenarioResults) {
                    $passed = ($result.status -eq "success") -and ($null -ne $result.extract_ms)
                    $detail = "variant=$($result.variant) status=$($result.status) extract_ms=$($result.extract_ms) mode_effective=$($result.mode_effective)"
                    $records += New-ValidationRecord -Scenario $scenarioName -Check "rich_smoke_stage_present" -Passed $passed -Detail $detail
                }
            }
            "rich_duplicate_pair" {
                foreach ($result in $scenarioResults) {
                    $passed = ($result.duplicates -gt 0) -and ($result.unique_tokens -lt $result.tokens_seen)
                    $detail = "variant=$($result.variant) unique=$($result.unique_tokens) tokens=$($result.tokens_seen) duplicates=$($result.duplicates)"
                    $records += New-ValidationRecord -Scenario $scenarioName -Check "pair_collapses_duplicates" -Passed $passed -Detail $detail
                }
                $parity = Test-CountParity -Results $scenarioResults -Variants @("ram_preserve", "disk_preserve", "auto_preserve")
                $records += New-ValidationRecord -Scenario $scenarioName -Check "count_parity" -Passed $parity.passed -Detail $parity.detail
            }
            "rich_mixed" {
                foreach ($result in $scenarioResults) {
                    $passed = ($result.status -eq "success") -and ($null -ne $result.extract_ms)
                    $detail = "variant=$($result.variant) status=$($result.status) extract_ms=$($result.extract_ms)"
                    $records += New-ValidationRecord -Scenario $scenarioName -Check "rich_extraction_stage_present" -Passed $passed -Detail $detail
                }
            }
        }
    }

    return $records
}

function Convert-ResultsToMarkdown {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,

        [Parameter(Mandatory = $true)]
        [string]$SuiteName,

        [Parameter(Mandatory = $true)]
        [object[]]$Results,

        [Parameter(Mandatory = $true)]
        [object[]]$Validations,

        [Parameter(Mandatory = $true)]
        [string]$Timestamp,

        [Parameter(Mandatory = $true)]
        [string]$ResultsJsonPath,

        [Parameter(Mandatory = $true)]
        [string]$ValidationsJsonPath,

        [Parameter(Mandatory = $true)]
        [string]$CommandLine
    )

    $version = Get-ProjectVersion -RepoRoot $RepoRoot
    $gitRevision = Get-GitRevision -RepoRoot $RepoRoot
    $dateLabel = Get-Date -Format "yyyy-MM-dd"
    $runTitle = if ($SuiteName -eq "all") { "Full Real Corpus Run" } else { "Real Corpus Run ($SuiteName)" }
    $failedValidations = @($Validations | Where-Object { -not $_.passed })
    $scenarioGroups = @($Results | Group-Object scenario | Sort-Object Name)

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("## $dateLabel - $runTitle")
    $lines.Add("")
    $lines.Add("- Version: ``$version``")
    $lines.Add("- Git revision: ``$gitRevision``")
    $lines.Add("- Scope: ``$SuiteName``")
    $lines.Add("- Trigger: manual benchmark run")
    $lines.Add("- Changes since previous run:")
    $lines.Add("  - fill this in if the run is being appended after code or corpus changes")
    $lines.Add("- Corpus notes:")
    $lines.Add("  - benchmark scenarios resolved from local ``testfiles/``")
    $lines.Add("- Environment notes:")
    $lines.Add("  - generated automatically by ``scripts/bench/run-real-corpus.ps1``")
    $lines.Add("- Commands:")
    $lines.Add("  - ``$CommandLine``")
    $lines.Add("- Results:")
    $lines.Add("  - outputs written to:")
    $lines.Add("    - ``$ResultsJsonPath``")
    $lines.Add("    - ``$ValidationsJsonPath``")
    $lines.Add("  - validation summary:")
    $lines.Add("    - ``$($Validations.Count)`` checks")
    $lines.Add("    - ``$($failedValidations.Count)`` failures")
    $lines.Add("  - key scenario results:")

    foreach ($group in $scenarioGroups) {
        $lines.Add("    - ``$($group.Name)``")
        $variants = @($group.Group | Sort-Object variant)
        foreach ($variant in $variants) {
            $parts = @(
                "``$($variant.variant)``"
                "``$($variant.elapsed_ms) ms``"
                "``$($variant.tokens_seen) tokens``"
                "``$($variant.unique_tokens) unique``"
                "``$($variant.duplicates) duplicates``"
                "``mode_effective=$($variant.mode_effective)``"
            )
            if ($null -ne $variant.extract_ms) {
                $parts += "``extract_ms=$($variant.extract_ms)``"
            }
            $lines.Add("      - " + ($parts -join ", "))
        }
    }

    $lines.Add("- Validations:")
    if ($failedValidations.Count -eq 0) {
        $lines.Add("  - passed")
        foreach ($validation in @($Validations | Sort-Object scenario, check)) {
            $lines.Add("  - ``$($validation.scenario)`` / ``$($validation.check)``: passed")
        }
    }
    else {
        $lines.Add("  - failed")
        foreach ($validation in @($Validations | Sort-Object scenario, check)) {
            $status = if ($validation.passed) { "passed" } else { "failed" }
            $lines.Add("  - ``$($validation.scenario)`` / ``$($validation.check)``: $status")
            $lines.Add("    - $($validation.detail)")
        }
    }

    $lines.Add("- Notes:")
    $lines.Add("  - artifact timestamp: ``$Timestamp``")
    $lines.Add("  - edit this section before pasting into ``docs/benchmark-history.md`` if the run needs more context")
    $lines.Add("- Follow-up:")
    $lines.Add("  - fill in the next action based on these results")

    return ($lines -join [Environment]::NewLine) + [Environment]::NewLine
}

$repoRoot = Get-RepoRoot
$allScenarios = Get-ScenarioDefinitions

if ($ListScenarios) {
    Show-ScenarioList -Scenarios $allScenarios
    exit 0
}

$corpus = Resolve-RealCorpus -RepoRoot $repoRoot -RequirePresent:$RequireCorpus

if ($ValidateCorpus) {
    if ($null -eq $corpus) {
        Write-Output "real corpus: missing (validation skipped)"
        exit 0
    }

    Validate-RealCorpus -Corpus $corpus
    exit 0
}

if ($null -eq $corpus) {
    throw "Real benchmark corpus is required to run scenarios. Populate testfiles/ or use -ListScenarios / -ValidateCorpus."
}

$cliBinary = Ensure-CliBinary -RepoRoot $repoRoot -RequestedCliPath $CliPath -SkipCliBuild:$SkipBuild
$selectedScenarios = Get-SelectedScenarios -AllScenarios $allScenarios -SelectedSuite $Suite

if ($selectedScenarios.Count -eq 0) {
    throw "No scenarios matched suite '$Suite'."
}

$outputRoot = Join-Path $repoRoot $OutputDir
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null

$results = New-Object System.Collections.Generic.List[object]
foreach ($scenario in $selectedScenarios) {
    $inputs = @($scenario.InputKeys | ForEach-Object { $corpus[$_] })
    foreach ($run in $scenario.Runs) {
        Write-Output "running $($scenario.Name)/$($run.Name)"
        $results.Add((Invoke-BenchmarkRun -CliBinary $cliBinary -Scenario $scenario -Run $run -Inputs $inputs -OutputRoot $outputRoot))
    }
}

$resultRows = @($results | ForEach-Object { $_ })
$validationRecords = Get-ValidationRecords -Results $resultRows
$validationRows = @($validationRecords | ForEach-Object { $_ })
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$jsonPath = Join-Path $outputRoot "results-$timestamp.json"
$csvPath = Join-Path $outputRoot "results-$timestamp.csv"
$validationJsonPath = Join-Path $outputRoot "validations-$timestamp.json"
$validationCsvPath = Join-Path $outputRoot "validations-$timestamp.csv"
$latestJsonPath = Join-Path $outputRoot "latest-results.json"
$latestCsvPath = Join-Path $outputRoot "latest-results.csv"
$latestValidationJsonPath = Join-Path $outputRoot "latest-validations.json"
$latestValidationCsvPath = Join-Path $outputRoot "latest-validations.csv"
$historyEntryPath = Join-Path $outputRoot "history-entry-$timestamp.md"
$latestHistoryEntryPath = Join-Path $outputRoot "latest-history-entry.md"

$resultRows | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
$resultRows | ConvertTo-Csv -NoTypeInformation | Set-Content -LiteralPath $csvPath -Encoding UTF8
$resultRows | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $latestJsonPath -Encoding UTF8
$resultRows | ConvertTo-Csv -NoTypeInformation | Set-Content -LiteralPath $latestCsvPath -Encoding UTF8

$validationRows | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $validationJsonPath -Encoding UTF8
$validationRows | ConvertTo-Csv -NoTypeInformation | Set-Content -LiteralPath $validationCsvPath -Encoding UTF8
$validationRows | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $latestValidationJsonPath -Encoding UTF8
$validationRows | ConvertTo-Csv -NoTypeInformation | Set-Content -LiteralPath $latestValidationCsvPath -Encoding UTF8

$commandParts = @(
    "pwsh -NoProfile -File scripts/bench/run-real-corpus.ps1"
    "-Suite $Suite"
)
if ($OutputDir -ne "target/real-corpus-bench") {
    $commandParts += "-OutputDir $OutputDir"
}
if ($CliPath) {
    $commandParts += "-CliPath $CliPath"
}
if ($SkipBuild) {
    $commandParts += "-SkipBuild"
}
if ($RequireCorpus) {
    $commandParts += "-RequireCorpus"
}
$historyEntryMarkdown = Convert-ResultsToMarkdown `
    -RepoRoot $repoRoot `
    -SuiteName $Suite `
    -Results $resultRows `
    -Validations $validationRows `
    -Timestamp $timestamp `
    -ResultsJsonPath $jsonPath `
    -ValidationsJsonPath $validationJsonPath `
    -CommandLine ($commandParts -join " ")

$historyEntryMarkdown | Set-Content -LiteralPath $historyEntryPath -Encoding UTF8
$historyEntryMarkdown | Set-Content -LiteralPath $latestHistoryEntryPath -Encoding UTF8

Write-Output "results_json=$jsonPath"
Write-Output "results_csv=$csvPath"
Write-Output "validations_json=$validationJsonPath"
Write-Output "validations_csv=$validationCsvPath"
Write-Output "history_entry_md=$historyEntryPath"
