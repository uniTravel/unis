import http from 'k6/http';
import { sleep, check } from 'k6';
import { uuidv4 } from './lib/k6-utils.js';

export const options = {
    vus: 3,
    duration: '30s',
    thresholds: {
        'http_req_failed': ['rate < 0.01'],
        'http_req_duration': ['max < 200', 'p(95) < 100'],
    },
}

const baseUrl = 'http://localhost:3000/v1/transaction'


export default function () {
    let aggId, comId, spanId, traceparent, params, data, res;

    aggId = uuidv4();
    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "account_code": "123456",
        "limit": 100000
    };

    res = http.post(`${baseUrl}/init`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    aggId = uuidv4();
    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "account_code": "123456"
    };

    res = http.post(`${baseUrl}/open`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "limit": 100000,
        "trans_limit": 100000,
        "balance": 0
    };

    res = http.post(`${baseUrl}/set_limit`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "limit": 200000
    };

    res = http.post(`${baseUrl}/change_limit`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "trans_limit": 200000
    };

    res = http.post(`${baseUrl}/set_trans_limit`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "amount": 100000
    }

    res = http.post(`${baseUrl}/deposit`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "amount": 100000
    };

    res = http.post(`${baseUrl}/withdraw`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "out_code": "123456",
        "amount": 100000
    };

    res = http.post(`${baseUrl}/transfer_in`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });

    comId = uuidv4().replace(/-/g, '');
    spanId = uuidv4().replace(/-/g, '').substring(0, 16);
    traceparent = `00-${comId}-${spanId}-01`;
    params = {
        headers: {
            'Content-Type': 'application/json',
            'traceparent': traceparent,
            'x-agg-id': aggId,
        },
    };
    data = {
        "in_code": "123456",
        "amount": 100000
    };

    res = http.post(`${baseUrl}/transfer_out`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });
}