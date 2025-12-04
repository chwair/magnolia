export function findDifferingPatterns(filenames) {
    if (filenames.length < 2) return [];
    const patterns = [];
    const firstFile = filenames[0];
    const numberRegex = /\d+/g;
    let match;
    const firstNumbers = [];
    while ((match = numberRegex.exec(firstFile)) !== null) {
        firstNumbers.push({ start: match.index, end: match.index + match[0].length, value: match[0] });
    }

    for (const numInfo of firstNumbers) {
        let varies = false;
        const values = new Set([numInfo.value]);
        for (let i = 1; i < filenames.length; i++) {
            const otherFile = filenames[i];
            const otherMatches = [...otherFile.matchAll(/\d+/g)];
            for (const otherMatch of otherMatches) {
                const beforeThis = firstFile.substring(Math.max(0, numInfo.start - 10), numInfo.start);
                const afterThis = firstFile.substring(numInfo.end, Math.min(firstFile.length, numInfo.end + 10));
                const beforeOther = otherFile.substring(Math.max(0, otherMatch.index - 10), otherMatch.index);
                const afterOther = otherFile.substring(otherMatch.index + otherMatch[0].length, Math.min(otherFile.length, otherMatch.index + otherMatch[0].length + 10));
                if (beforeThis === beforeOther || afterThis === afterOther) {
                    if (otherMatch[0] !== numInfo.value) varies = true;
                    values.add(otherMatch[0]);
                    break;
                }
            }
        }
        if (varies && values.size > 1) {
            patterns.push({ 
                contextBefore: firstFile.substring(Math.max(0, numInfo.start - 15), numInfo.start), 
                contextAfter: firstFile.substring(numInfo.end, Math.min(firstFile.length, numInfo.end + 15)) 
            });
        }
    }
    return patterns;
}

export function renderFilenameWithHighlights(filename, highlightPatterns) {
    if (highlightPatterns.length === 0) return [{ text: filename, highlighted: false }];
    const highlights = [];
    const numberRegex = /\d+/g;
    let match;
    while ((match = numberRegex.exec(filename)) !== null) {
        for (const pattern of highlightPatterns) {
            const beforeInFile = filename.substring(Math.max(0, match.index - pattern.contextBefore.length), match.index);
            const afterInFile = filename.substring(match.index + match[0].length, Math.min(filename.length, match.index + match[0].length + pattern.contextAfter.length));
            if (beforeInFile.endsWith(pattern.contextBefore) || afterInFile.startsWith(pattern.contextAfter)) {
                highlights.push({ start: match.index, end: match.index + match[0].length });
                break;
            }
        }
    }
    highlights.sort((a, b) => a.start - b.start);
    const parts = [];
    let lastEnd = 0;
    for (const hl of highlights) {
        if (hl.start > lastEnd) parts.push({ text: filename.substring(lastEnd, hl.start), highlighted: false });
        parts.push({ text: filename.substring(hl.start, hl.end), highlighted: true });
        lastEnd = hl.end;
    }
    if (lastEnd < filename.length) parts.push({ text: filename.substring(lastEnd), highlighted: false });
    return parts.length > 0 ? parts : [{ text: filename, highlighted: false }];
}

export function getHighlightedNumber(filename, highlightPatterns) {
    const parts = renderFilenameWithHighlights(filename, highlightPatterns);
    const highlighted = parts.find(p => p.highlighted);
    return highlighted ? highlighted.text : null;
}

export function formatFileSize(bytes) {
    if (!bytes) return "";
    const mb = bytes / 1024 / 1024;
    if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
    return `${mb.toFixed(1)} MB`;
}

export function sortFiles(files, sortOrder) {
    return [...files].sort((a, b) => {
        const nameA = a.name.toLowerCase();
        const nameB = b.name.toLowerCase();
        return sortOrder === "asc" 
            ? nameA.localeCompare(nameB, undefined, { numeric: true })
            : nameB.localeCompare(nameA, undefined, { numeric: true });
    });
}

export function sortSelectionOrder(selectionOrder, files, sortOrder) {
    return [...selectionOrder].sort((a, b) => {
        const fileA = files.find(f => f.index === a);
        const fileB = files.find(f => f.index === b);
        if (!fileA || !fileB) return 0;
        const nameA = fileA.name.toLowerCase();
        const nameB = fileB.name.toLowerCase();
        return sortOrder === "asc"
            ? nameA.localeCompare(nameB, undefined, { numeric: true })
            : nameB.localeCompare(nameA, undefined, { numeric: true });
    });
}
