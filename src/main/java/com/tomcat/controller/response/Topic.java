package com.tomcat.controller.response;

import lombok.Data;

import java.util.List;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.response
 * @className: Topic
 * @author: tomcat
 * @description: TODO
 * @date: 2023/8/30 1:56 PM
 * @version: 1.0
 */
@Data
public class Topic {
    String topic;
    List<String> objective;
}
