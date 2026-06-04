import init, { WasmDocument, version } from './pkg/mrio2_web.js';

let wasmDoc = null;

async function initWasm() {
  await init();
  document.getElementById('app-version').textContent = `v${version()}`;
}

initWasm().catch(err => {
  console.error('Failed to initialize WASM:', err);
  ot.toast('Failed to load editor. Please refresh the page.', 'Error', { variant: 'danger' });
});

const dropZone = document.getElementById('drop-zone');
const fileInput = document.getElementById('file-input');
const browseBtn = document.getElementById('browse-btn');
const editor = document.getElementById('editor');
const fileInfo = document.getElementById('file-info');
const statsContent = document.getElementById('stats-content');

browseBtn.addEventListener('click', () => fileInput.click());

fileInput.addEventListener('change', (e) => {
  if (e.target.files.length > 0) {
    handleFile(e.target.files[0]);
  }
});

dropZone.addEventListener('dragover', (e) => {
  e.preventDefault();
  dropZone.classList.add('dragover');
});

dropZone.addEventListener('dragleave', () => {
  dropZone.classList.remove('dragover');
});

dropZone.addEventListener('drop', (e) => {
  e.preventDefault();
  dropZone.classList.remove('dragover');
  if (e.dataTransfer.files.length > 0) {
    handleFile(e.dataTransfer.files[0]);
  }
});

async function handleFile(file) {
  const reader = new FileReader();
  reader.onload = async (e) => {
    const content = e.target.result;
    try {
      wasmDoc = new WasmDocument(content, file.name);
      dropZone.style.display = 'none';
      editor.style.display = 'grid';
      fileInfo.textContent = file.name;
      updateStats();
      ot.toast('File loaded successfully', 'Success', { variant: 'success' });
    } catch (err) {
      ot.toast(err.toString(), 'Failed to parse file', { variant: 'danger' });
    }
  };
  reader.readAsText(file);
}

function updateStats() {
  if (!wasmDoc) return;

  try {
    const stats = wasmDoc.get_stats();
    renderStats(stats);
  } catch (err) {
    statsContent.innerHTML = `<div role="alert" data-variant="error"><strong>Error:</strong> ${err}</div>`;
  }
}

function renderStats(stats) {
  statsContent.removeAttribute('aria-busy');
  statsContent.removeAttribute('data-spinner');

  let html = '';

  const crsDisplay = stats.crs && stats.crs !== 'none'
    ? stats.crs.split('/').pop()
    : 'none';

  html += '<div class="stats-grid">';
  html += `<div class="stat-card"><div class="label">Format</div><div class="value">${stats.format_name} v${stats.version}</div></div>`;
  html += `<div class="stat-card"><div class="label">Objects</div><div class="value">${stats.total_objects.toLocaleString()}</div></div>`;
  html += `<div class="stat-card"><div class="label">Vertices</div><div class="value">${stats.total_vertices.toLocaleString()}</div></div>`;
  html += `<div class="stat-card"><div class="label">CRS</div><div class="value">${crsDisplay}</div></div>`;
  html += '</div>';

  if (stats.extensions.length > 0) {
    html += '<div class="stats-section"><h4>Extensions</h4><ul class="ext-list">';
    for (const [name, url] of stats.extensions) {
      html += `<li><strong>${name}</strong>`;
      if (url) {
        html += ` — <a href="${url}" target="_blank" rel="noopener">${url}</a>`;
      }
      html += '</li>';
    }
    html += '</ul></div>';
  }

  if (stats.object_type_counts.length > 0 || stats.other_object_types.length > 0) {
    html += '<div class="stats-section"><h4>Object Types</h4><ul class="type-list">';
    for (const [type, count] of stats.object_type_counts) {
      html += `<li><span>${type}</span><span class="badge">${count}</span></li>`;
    }
    for (const [type, count] of stats.other_object_types) {
      html += `<li><span>${type}</span><span class="badge" data-variant="secondary">${count}</span></li>`;
    }
    html += '</ul></div>';
  }

  if (stats.attribute_inventory.length > 0) {
    html += '<div class="stats-section"><h4>Attributes</h4><ul class="attr-list">';
    for (const [name, count, sample] of stats.attribute_inventory) {
      const pct = stats.objects_with_attrs > 0
        ? Math.round((count / stats.objects_with_attrs) * 100)
        : 0;
      html += `<li><span>${name} <span class="attr-sample">${sample}</span></span><span class="badge" data-variant="secondary">${count} (${pct}%)</span></li>`;
    }
    html += '</ul></div>';
  }

  statsContent.innerHTML = html;
}

document.querySelectorAll('[data-op]').forEach(btn => {
  btn.addEventListener('click', async () => {
    if (!wasmDoc) {
      ot.toast('Please load a file first', 'No file loaded', { variant: 'warning' });
      return;
    }

    const op = btn.dataset.op;
    const dialogId = btn.dataset.dialog;

    if (dialogId) {
      const dialog = document.getElementById(dialogId);
      if (op === 'remove_attribute' || op === 'rename_attribute') {
        populateAttributeSelects();
      }
      if (op === 'download') {
        const filenameInput = document.getElementById('download-filename');
        const baseName = wasmDoc.get_filename().replace(/\.(city\.jsonl?|jsonl?)$/i, '');
        const format = wasmDoc.get_output_format();
        const ext = format === 'cityjsonseq' ? 'city.jsonl' : 'city.json';
        filenameInput.value = `${baseName}.modified.${ext}`;
      }
      dialog.showModal();
    } else {
      await runOperation(op, '');
    }
  });
});

function populateAttributeSelects() {
  if (!wasmDoc) return;

  try {
    const attrs = wasmDoc.get_attributes();
    const deleteSelect = document.getElementById('delete-attr-select');
    const renameSelect = document.getElementById('rename-attr-select');

    deleteSelect.innerHTML = '';
    renameSelect.innerHTML = '';

    for (const attr of attrs) {
      deleteSelect.innerHTML += `<option value="${attr}">${attr}</option>`;
      renameSelect.innerHTML += `<option value="${attr}">${attr}</option>`;
    }

    if (attrs.length === 0) {
      deleteSelect.innerHTML = '<option disabled>No attributes found</option>';
      renameSelect.innerHTML = '<option disabled>No attributes found</option>';
    }
  } catch (err) {
    ot.toast(err.toString(), 'Failed to load attributes', { variant: 'danger' });
  }
}

async function runOperation(op, param) {
  if (!wasmDoc) return;

  try {
    if (op === 'validate_schema') {
      const validationDialog = document.getElementById('validation-dialog');
      const validationTitle = document.getElementById('validation-title');
      const validationOutput = document.getElementById('validation-output');

      validationOutput.innerHTML = '<div class="validation-item"><div class="validation-icon info">⏳</div><div class="validation-content">Fetching extension schemas...</div></div>';
      validationTitle.textContent = 'Schema Validation';
      validationTitle.style.color = 'var(--muted-foreground)';
      validationDialog.showModal();

      let extensionSchemasJson = '[]';
      const extUrls = wasmDoc.get_extension_urls();
      if (extUrls) {
        const entries = extUrls.split('\n').map(e => {
          const sep = e.indexOf('|');
          return { name: e.substring(0, sep), url: e.substring(sep + 1) };
        });

        const fetches = entries.map(async (entry) => {
          try {
            const resp = await fetch(entry.url);
            if (!resp.ok) {
              return { name: entry.name, schema: `FETCH_ERROR: HTTP ${resp.status}` };
            }
            const schema = await resp.text();
            return { name: entry.name, schema };
          } catch (err) {
            return { name: entry.name, schema: `FETCH_ERROR: ${err.message}` };
          }
        });

        const results = await Promise.all(fetches);
        extensionSchemasJson = JSON.stringify(results);
      }

      validationOutput.innerHTML = '<div class="validation-item"><div class="validation-icon info">⏳</div><div class="validation-content">Validating...</div></div>';
      const result = wasmDoc.validate_with_extensions(extensionSchemasJson);

      renderValidationOutput(result);

      if (result.is_error) {
        validationTitle.textContent = 'Schema Validation — Errors';
        validationTitle.style.color = 'var(--danger)';
      } else if (result.summary.includes('[warning]')) {
        validationTitle.textContent = 'Schema Validation — Warnings';
        validationTitle.style.color = 'var(--warning)';
      } else {
        validationTitle.textContent = 'Schema Validation — OK';
        validationTitle.style.color = 'var(--success)';
      }
    } else {
      const result = wasmDoc.run_operation(op, param);
      if (result.is_error) {
        ot.toast(result.summary, 'Operation failed', { variant: 'danger' });
      } else {
        ot.toast(result.summary, 'Success', { variant: 'success' });
        updateStats();
      }
    }
  } catch (err) {
    ot.toast(err.toString(), 'Operation error', { variant: 'danger' });
  }
}

function renderValidationOutput(result) {
  const output = document.getElementById('validation-output');
  const lines = result.summary.split('\n');
  let html = '';
  let currentDetails = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    
    if (line.startsWith('    ')) {
      currentDetails.push(line.substring(4));
      continue;
    }

    if (currentDetails.length > 0 && html.length > 0) {
      html = html.replace('</div></div>', 
        `<div class="validation-details">${currentDetails.join('\n')}</div></div></div>`);
      currentDetails = [];
    }

    let icon = '';
    let iconClass = '';
    let text = line;

    if (line.startsWith(' ✓ ')) {
      icon = '✓';
      iconClass = 'valid';
      text = line.substring(3);
    } else if (line.startsWith(' ✗ ')) {
      icon = '✗';
      iconClass = line.includes('[warning]') ? 'warning' : 'error';
      text = line.substring(3);
    } else if (line.startsWith(' ! ')) {
      icon = '!';
      iconClass = 'error';
      text = line.substring(3);
    } else if (line.startsWith(' ⏳ ')) {
      icon = '⏳';
      iconClass = 'info';
      text = line.substring(3);
    } else {
      continue;
    }

    text = text.replace(/\s*\[(error|warning)\]$/, '');

    html += `<div class="validation-item">
      <div class="validation-icon ${iconClass}">${icon}</div>
      <div class="validation-content">
        <div class="validation-criterion">${text}</div>
      </div>
    </div>`;
  }

  if (currentDetails.length > 0 && html.length > 0) {
    html = html.replace('</div></div>', 
      `<div class="validation-details">${currentDetails.join('\n')}</div></div></div>`);
  }

  output.innerHTML = html;
}

document.getElementById('delete-attr-dialog').addEventListener('close', async function(e) {
  if (this.returnValue === 'confirm') {
    const attr = document.getElementById('delete-attr-select').value;
    if (attr) {
      await runOperation('remove_attribute', attr);
    }
  }
});

document.getElementById('rename-attr-dialog').addEventListener('close', async function(e) {
  if (this.returnValue === 'confirm') {
    const oldName = document.getElementById('rename-attr-select').value;
    const newName = document.getElementById('rename-new-name').value.trim();
    if (oldName && newName) {
      await runOperation('rename_attribute', `${oldName}|${newName}`);
      document.getElementById('rename-new-name').value = '';
    } else {
      ot.toast('Please fill in both fields', 'Invalid input', { variant: 'warning' });
    }
  }
});

document.getElementById('csv-dialog').addEventListener('close', async function(e) {
  if (this.returnValue === 'confirm') {
    const fileInput = document.getElementById('csv-file-input');
    if (fileInput.files.length > 0) {
      const reader = new FileReader();
      reader.onload = async (e) => {
        await runOperation('add_from_csv', e.target.result);
        fileInput.value = '';
      };
      reader.readAsText(fileInput.files[0]);
    } else {
      ot.toast('Please select a CSV file', 'No file selected', { variant: 'warning' });
    }
  }
});

document.getElementById('epsg-dialog').addEventListener('close', async function(e) {
  if (this.returnValue === 'confirm') {
    const epsg = document.getElementById('epsg-input').value.trim();
    if (epsg) {
      await runOperation('set_epsg', epsg);
      document.getElementById('epsg-input').value = '';
    } else {
      ot.toast('Please enter an EPSG code', 'Invalid input', { variant: 'warning' });
    }
  }
});

document.getElementById('download-dialog').addEventListener('close', async function(e) {
  if (this.returnValue === 'confirm') {
    const format = document.querySelector('input[name="download-format"]:checked').value;
    const filename = document.getElementById('download-filename').value.trim();

    if (!filename) {
      ot.toast('Please enter a filename', 'Invalid input', { variant: 'warning' });
      return;
    }

    try {
      const content = wasmDoc.serialize(format);
      const blob = new Blob([content], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      ot.toast('File downloaded successfully', 'Success', { variant: 'success' });
    } catch (err) {
      ot.toast(err.toString(), 'Download failed', { variant: 'danger' });
    }
  }
});

document.getElementById('reset-btn').addEventListener('click', () => {
  wasmDoc = null;
  editor.style.display = 'none';
  dropZone.style.display = 'flex';
  fileInfo.textContent = '';
  fileInput.value = '';
  statsContent.innerHTML = '<p>Loading file information...</p>';
  statsContent.setAttribute('aria-busy', 'true');
  statsContent.setAttribute('data-spinner', 'large overlay');
});
