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

const baseUrl = 'http://localhost:3000/api/v1/json/account'

export default function () {
    let comId, spanId, traceparent, params, data, res;
    const aggId = uuidv4();

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
        "code": "123456",
        "owner": "张三"
    };

    res = http.post(`${baseUrl}/create`, JSON.stringify(data), params);
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
        "verified_by": "李四",
        "verified": true,
    };

    res = http.post(`${baseUrl}/verify`, JSON.stringify(data), params);
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
        "approved_by": "王五",
        "approved": true,
        "limit": 10000
    };

    res = http.post(`${baseUrl}/approve`, JSON.stringify(data), params);
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
        "limit": 100000
    };

    res = http.post(`${baseUrl}/limit`, JSON.stringify(data), params);
    check(res, { "status is 200": (r) => r.status === 200 });
}