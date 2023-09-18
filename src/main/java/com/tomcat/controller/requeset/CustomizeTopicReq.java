package com.tomcat.controller.requeset;

import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.requeset
 * @className: CustomizeTopicReq
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/15 4:23 PM
 * @version: 1.0
 */
@Data
public class CustomizeTopicReq {
    private String assistant_role;
    private String user_role;
    private String topic;
}
