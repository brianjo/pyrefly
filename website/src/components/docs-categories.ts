/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

import { docsSidebar } from '../../sidebars';

interface SidebarItem {
    type: string;
    label: string;
    id?: string;
    items?: string[];
    description?: string;
}

interface DocsCategory {
    docId?: string;
    href: string;
    label: string;
    type: string;
    description?: string;
}

const docsCategories: DocsCategory[] = docsSidebar.map((item: SidebarItem) => {
    const label = item.label;
    const id = item.type === 'doc' ? item.id! : item.items![0];
    let href = `/en/docs/${id}`;
    if (href.endsWith('/index')) href = href.substring(0, href.length - 6);

    if (item.type === 'category' && item.description !== undefined) {
        return { type: 'link', href, label, description: item.description };
    }
    return { type: 'link', docId: id, href, label };
});

export default docsCategories;
