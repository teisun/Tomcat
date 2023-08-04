package com.tomcat.domain;

import javax.persistence.*;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

@Entity
public class User {

  @Id
  @GeneratedValue
  private Long id;

  @Column(unique=true)
  private String username;

  private String password;
  @Column(unique=true)
  private String email;
  @Column(unique=true)
  private String phoneNum;
  @ElementCollection
  private List<String> deviceIds = new ArrayList<>();

  public User(Long id, String username, String password, String email, String phoneNum, String deviceId) {
    this.id = id;
    this.username = username;
    this.password = password;
    this.email = email;
    this.phoneNum = phoneNum;
    this.deviceIds.add(deviceId);
  }

  public User() {

  }

  public String getPhoneNum() {
    return phoneNum;
  }

  public void setPhoneNum(String phoneNum) {
    this.phoneNum = phoneNum;
  }

  public void addDeviceId(String deviceId) {
    this.deviceIds.add(deviceId);
  }

  public void removeDeviceId(String deviceId) {
    this.deviceIds.remove(deviceId);
  }

  public List<String> getDeviceIds() {
    return Collections.unmodifiableList(deviceIds);
  }

  public Long getId() {
    return id;
  }

  public void setId(Long id) {
    this.id = id;
  }

  public String getUsername() {
    return username;
  }

  public void setUsername(String username) {
    this.username = username;
  }

  public String getPassword() {
    return password;
  }

  public void setPassword(String password) {
    this.password = password;
  }

  public String getEmail() {
    return email;
  }

  public void setEmail(String email) {
    this.email = email;
  }

  // 省略getter/setter
}