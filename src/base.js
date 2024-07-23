class Coaxial {
    constructor(seed = null) {
        // TODO prefill state with things
        // idk where from
        this.state = {};
        this.stateChangeListeners = {};

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
                    this.state[field] = value;

                    // TODO delete this
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
     * Add a listener for a state.
     * The listener will be called when the state is updated.
     *
     * @param {string|string[]} id
     * @param {(value: any) => void} id
     */
    onStateChange(id, closure) {
        if (Array.isArray(id)) {
            const ids = id;
            // we call the closure with All of the states they need
            for (const id of ids) {
                this.onStateChange(id, v => {
                    const params = ids.map(i => i === id ? v : this.state[i]);
                    closure(params);
                });
            }

            return;
        }

        if (this.stateChangeListeners[id] === undefined) {
            this.stateChangeListeners[id] = [closure];
        } else {
            this.stateChangeListeners[id].push(closure);
        }
    }

    callOnChange(id, value) {
        if (this.stateChangeListeners[id] === undefined) {
            return;
        }

        for (const closure of this.stateChangeListeners[id]) {
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
