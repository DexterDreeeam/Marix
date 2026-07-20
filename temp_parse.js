const fs = require('fs');

function parseRustFile(path) {
    const content = fs.readFileSync(path, 'utf8');
    const lines = content.split('\n');
    const elements = [];
    
    let i = 0;
    while (i < lines.length) {
        const line = lines[i];
        const pubMatch = line.match(/^(?:#\[.*?\]\s*)*?(pub(?:\(.*?\))?)\s+(struct|enum|trait|fn|type|const|static)\s+([A-Za-z0-9_]+)/);
        if (pubMatch) {
            const kind = pubMatch[2] === 'fn' ? 'function' : pubMatch[2] === 'type' ? 'type-alias' : pubMatch[2];
            const name = pubMatch[3];
            const start = i;
            let end = start;
            if (!line.includes(';')) {
                let braceCount = 0;
                let started = false;
                for (let j = start; j < lines.length; j++) {
                    const l = lines[j];
                    braceCount += (l.match(/\{/g) || []).length;
                    braceCount -= (l.match(/\}/g) || []).length;
                    if (l.includes('{')) started = true;
                    if (started && braceCount === 0) {
                        end = j;
                        break;
                    }
                    if (!started && l.includes(';')) {
                        end = j;
                        break;
                    }
                }
            }
            // scan backwards for doc comments and attributes
            let actualStart = start;
            while (actualStart > 0 && (lines[actualStart-1].trim().startsWith('///') || lines[actualStart-1].trim().startsWith('#['))) {
                actualStart--;
            }
            elements.push({ name, type: kind, sourcePath: path, lineStart: actualStart + 1, lineEnd: end + 1 });
        }
        
        // Also capture impl blocks for known elements (we'll match by name later)
        const implMatch = line.match(/^impl(?:<.*?>)?\s+(?:.*?\s+for\s+)?([A-Za-z0-9_]+)/);
        if (implMatch && !line.includes(';')) {
            const name = implMatch[1];
            const start = i;
            let braceCount = 0;
            let started = false;
            let end = start;
            for (let j = start; j < lines.length; j++) {
                const l = lines[j];
                braceCount += (l.match(/\{/g) || []).length;
                braceCount -= (l.match(/\}/g) || []).length;
                if (l.includes('{')) started = true;
                if (started && braceCount === 0) {
                    end = j;
                    break;
                }
            }
            // scan backwards for attributes
            let actualStart = start;
            while (actualStart > 0 && lines[actualStart-1].trim().startsWith('#[')) {
                actualStart--;
            }
            elements.push({ implFor: name, sourcePath: path, lineStart: actualStart + 1, lineEnd: end + 1 });
        }
        
        i++;
    }
    return elements;
}

const paths = [
    'src/protocol/tool/category.rs',
    'src/server/model/deepseek/backend.rs',
    'src/server/model/deepseek/request.rs',
    'src/server/model/deepseek/stream.rs',
    'src/tool/native/sys/os_env.rs',
    'src/tool/native/shell/powershell.rs',
    'src/tool/native/coding/get_code_outline.rs',
    'src/tool/native/coding/replace_in_file.rs',
    'src/tool/native/process/start_process.rs',
    'src/tool/native/process/read_process_output.rs',
    'src/tool/native/process/stop_process.rs',
    'src/tool/native/process/output.rs',
    'src/tool/native/process/process_registry.rs',
    'src/tool/native/web/web_fetch.rs',
    'src/tool/native/web/web_search.rs'
];

const results = paths.map(p => parseRustFile(p));
console.log(JSON.stringify(results.flat(), null, 2));
