const fs = require('fs');
const path = require('path');

const NEW_ELEMENTS = require('./temp_parse_results.json');

const UPDATES = [
    {
        module: 'src/protocol/tool',
        status: 'modified',
        modified: ['category.rs'],
        elements: [
            {
                name: 'ToolCategory',
                type: 'enum',
                status: 'modified',
                segments: [{
                    sourcePath: 'src/protocol/tool/category.rs',
                    addedLines: [{lineStart: 7, lineEnd: 7}, {lineStart: 11, lineEnd: 11}],
                    modifiedLines: []
                }]
            }
        ]
    },
    {
        module: 'src/server/model/deepseek',
        status: 'modified',
        modified: ['backend.rs', 'request.rs', 'stream.rs'],
        elements: [
            {
                name: 'DeepseekBackend',
                type: 'struct',
                status: 'modified',
                segments: [
                    {
                        sourcePath: 'src/server/model/deepseek/backend.rs',
                        addedLines: [{lineStart: 75, lineEnd: 75}],
                        modifiedLines: [{lineStart: 94, lineEnd: 94}, {lineStart: 124, lineEnd: 125}]
                    },
                    {
                        sourcePath: 'src/server/model/deepseek/request.rs',
                        addedLines: [{lineStart: 19, lineEnd: 19}, {lineStart: 21, lineEnd: 27}],
                        modifiedLines: [{lineStart: 28, lineEnd: 28}, {lineStart: 33, lineEnd: 34}, {lineStart: 36, lineEnd: 36}]
                    },
                    {
                        sourcePath: 'src/server/model/deepseek/stream.rs',
                        addedLines: [{lineStart: 14, lineEnd: 14}],
                        modifiedLines: [{lineStart: 23, lineEnd: 23}]
                    }
                ]
            }
        ]
    },
    {
        module: 'src/tool/native/sys',
        status: 'modified',
        modified: ['os_env.rs'],
        elements: [
            {
                name: 'OsEnv',
                type: 'struct',
                status: 'modified',
                segments: [
                    {
                        sourcePath: 'src/tool/native/sys/os_env.rs',
                        addedLines: [],
                        modifiedLines: [{lineStart: 44, lineEnd: 46}]
                    }
                ]
            }
        ]
    },
    {
        module: 'src/tool/native',
        status: 'modified',
        modified: ['mod.rs'],
        childModules: [
            {path: 'src/tool/native/coding', name: 'coding', purpose: 'Native coding tool implementations.'},
            {path: 'src/tool/native/process', name: 'process', purpose: 'Native process tool implementations.'},
            {path: 'src/tool/native/web', name: 'web', purpose: 'Native web tool implementations.'}
        ]
    },
    {
        module: 'src/tool/native/shell',
        status: 'modified',
        modified: ['mod.rs'],
        added: ['powershell.rs'],
        deleted: ['powershell_exec.rs'],
        elements: [
            {
                name: 'PowerShell',
                type: 'struct',
                status: 'added',
                segments: [
                    {
                        sourcePath: 'src/tool/native/shell/powershell.rs',
                        addedLines: [],
                        modifiedLines: []
                    }
                ]
            }
        ]
    },
    {
        module: 'src/tool/native/coding',
        status: 'added',
        added: ['.', 'get_code_outline.rs', 'replace_in_file.rs', 'mod.rs'],
        elements: [
            {
                name: 'GetCodeOutline',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/coding/get_code_outline.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'ReplaceInFile',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/coding/replace_in_file.rs', addedLines: [], modifiedLines: []}]
            }
        ]
    },
    {
        module: 'src/tool/native/process',
        status: 'added',
        added: ['.', 'start_process.rs', 'read_process_output.rs', 'stop_process.rs', 'output.rs', 'process_registry.rs', 'mod.rs'],
        elements: [
            {
                name: 'StartProcess',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/start_process.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'ReadProcessOutput',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/read_process_output.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'StopProcess',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/stop_process.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'Output',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/output.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'read',
                type: 'function',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/output.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'start',
                type: 'function',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/process_registry.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'read_output',
                type: 'function',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/process_registry.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'stop',
                type: 'function',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/process/process_registry.rs', addedLines: [], modifiedLines: []}]
            }
        ]
    },
    {
        module: 'src/tool/native/web',
        status: 'added',
        added: ['.', 'web_fetch.rs', 'web_search.rs', 'mod.rs'],
        elements: [
            {
                name: 'WebFetch',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/web/web_fetch.rs', addedLines: [], modifiedLines: []}]
            },
            {
                name: 'WebSearch',
                type: 'struct',
                status: 'added',
                segments: [{sourcePath: 'src/tool/native/web/web_search.rs', addedLines: [], modifiedLines: []}]
            }
        ]
    }
];

// Parents also need status update
const PARENT_UPDATES = [
    {module: 'src/protocol'},
    {module: 'src/server/model'},
    {module: 'src/tool'},
];

for (let parent of PARENT_UPDATES) {
    let update = UPDATES.find(u => u.module === parent.module);
    if (!update) {
        UPDATES.push({
            module: parent.module,
            status: 'modified',
            modified: ['.']
        });
    } else {
        if (!update.modified) update.modified = [];
        if (!update.modified.includes('.')) update.modified.push('.');
    }
}

for (let u of UPDATES) {
    let metaPath = u.module.replace('src', 'src_meta') + '/design.json';
    if (!fs.existsSync(metaPath)) {
        if (!fs.existsSync(path.dirname(metaPath))) fs.mkdirSync(path.dirname(metaPath), {recursive: true});
        fs.writeFileSync(metaPath, JSON.stringify({
            schemaVersion: 1,
            module: {path: u.module, name: path.basename(u.module), purpose: "New module", changeStatus: u.status},
            added: [], modified: [], renamed: [], deleted: [], childModules: [], elements: []
        }));
    }
    
    let doc = JSON.parse(fs.readFileSync(metaPath, 'utf8'));
    
    if (u.status) doc.module.changeStatus = u.status;
    
    if (u.added) {
        doc.added = doc.added || [];
        for (let a of u.added) if (!doc.added.includes(a)) doc.added.push(a);
    }
    if (u.modified) {
        doc.modified = doc.modified || [];
        for (let m of u.modified) if (!doc.modified.includes(m)) doc.modified.push(m);
    }
    if (u.deleted) {
        doc.deleted = doc.deleted || [];
        for (let d of u.deleted) if (!doc.deleted.includes(d)) doc.deleted.push(d);
    }
    
    if (u.childModules) {
        doc.childModules = doc.childModules || [];
        for (let cm of u.childModules) {
            if (!doc.childModules.find(c => c.path === cm.path)) {
                doc.childModules.push(cm);
            }
        }
    }
    
    if (u.elements) {
        doc.elements = doc.elements || [];
        for (let el of u.elements) {
            let targetEl = doc.elements.find(e => e.name === el.name && e.type === el.type);
            if (!targetEl) {
                targetEl = {
                    name: el.name,
                    type: el.type,
                    source_depth: u.module.split('/').length,
                    changeStatus: el.status,
                    codeSegments: []
                };
                doc.elements.push(targetEl);
            } else {
                targetEl.changeStatus = el.status;
            }
            
            // Build code segments from temp_parse_results
            let segments = [];
            for (let seg of el.segments) {
                let parsed = NEW_ELEMENTS.filter(e => e.sourcePath === seg.sourcePath && (e.name === el.name || e.implFor === el.name));
                for (let p of parsed) {
                    // Only keep addedLines and modifiedLines that fall within this segment's range
                    let segAdded = (seg.addedLines || []).filter(l => l.lineStart >= p.lineStart && l.lineEnd <= p.lineEnd);
                    let segModified = (seg.modifiedLines || []).filter(l => l.lineStart >= p.lineStart && l.lineEnd <= p.lineEnd);
                    segments.push({
                        sourcePath: p.sourcePath,
                        lineStart: p.lineStart,
                        lineEnd: p.lineEnd,
                        language: 'rust',
                        addedLines: segAdded,
                        modifiedLines: segModified
                    });
                }
            }
            if (segments.length > 0) {
                targetEl.codeSegments = segments;
            }
        }
    }

    if (u.module === 'src/tool/native/shell') {
        doc.elements = doc.elements.filter(e => e.name !== 'PowerShellExec');
    }
    
    fs.writeFileSync(metaPath, JSON.stringify(doc, null, 2) + '\n');
}
