class Coaxial {
    constructor(seed = null) {
        this.changeListeners = {};

        const url = new URL(window.location);
        if (seed) url.searchParams.append('coaxial-seed', seed);

        this.conn = new WebSocket(url);
        this.conn.onopen = () => {
            console.log('Connected.');
            /* this.send({t: 'init'}); */
        };
        this.conn.onmessage = async (e) => {
            const msg = JSON.parse(e.data);

            if (msg.t === 'Update') {
                for (const [field, value] of msg.fields) {
                    document.querySelectorAll(`[data-coaxial-id="${field}"]`).forEach(el => {
                        el.innerHTML = value;
                    });
                    document.querySelectorAll(`[coax-change-${field}]`).forEach(el => {
                        let name = el.getAttribute(`coax-change-${field}`);
                        el[name] = value;
                    });

                    this.callOnChange(field, value);
                }
            }
        };
    }

    callClosure(closure) {
        this.send({
            t: 'Closure',
            closure
        });
    }

    setValue(id, value) {
        this.send({
            t: 'Set',
            id,
            value
        });
    }

    onEvent(name, params) {
        this.send({
            t: 'Event',
            name,
            params
        });
    }

    send(body) {
        this.conn.send(JSON.stringify(body));
    }

    /**
     * @param {string} id
     * @param {(value: any) => void} id
     */
    onChange(id, closure) {
        if (this.changeListeners[id] === undefined) {
            this.changeListeners = [closure];
        } else {
            this.changeListeners.push(closure);
        }
    }

    callOnChange(id, value) {
        if (this.changeListeners[id] === undefined) {
            return;
        }

        for (const closure of this.changeListeners[id]) {
            closure(value);
        }
    }
}

document.addEventListener("DOMContentLoaded", () => {
    window.Coaxial = new Coaxial('__internal__coaxialSeed');
});

// https://stackoverflow.com/a/34519193
function stringifyEvent(e) {
    const obj = {};
    for (let k in e) {
        obj[k] = e[k];
    }
    return JSON.stringify(obj, (k, v) => {
        if (v instanceof Node) return 'Node';
        if (v instanceof Window) return 'Window';
        return v;
    }, ' ');
}
